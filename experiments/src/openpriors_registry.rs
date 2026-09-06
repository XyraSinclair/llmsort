//! Disk-backed instrument registry for the OpenPriors collaborative ledger.
//!
//! Instruments are permissionless, content-addressed data
//! ([`crate::openpriors`] invariant 3): registering one validates its shape,
//! computes its hash, persists it, and records an `(owner, name) → hash`
//! alias. The registry is append-only — a name can never silently re-point
//! at different bytes; re-registering the same content is idempotent.
//!
//! Layout under the registry directory:
//! - `instruments/<hash>.json` — one file per instrument, named by its
//!   content hash (verified on load; a mismatch is corruption, not repair
//!   material).
//! - `registrations.jsonl` — the append-only alias ledger.
//!
//! The three live instruments seed themselves at daemon startup
//! ([`Registry::seed_builtins`]) under the platform account. Their template
//! bytes are DERIVED from the engine's own renderers — rendered with the
//! contract's slot literals as bodies — so registry bytes cannot drift from
//! engine bytes: a changed engine prompt is a new instrument hash, exactly
//! as it should be.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::openpriors::{
    AccountId, ContentHash, Currency, Instrument, InstrumentRegistration, ValidateError,
};

/// The reserved owner of the seeded builtin instruments.
pub const PLATFORM_ACCOUNT: &str = "openpriors";

const MAX_NAME_BYTES: usize = 120;
const MAX_OWNER_BYTES: usize = 120;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("invalid instrument — {0}")]
    Invalid(#[from] ValidateError),
    #[error("name must be 1-120 bytes and owner 1-120 bytes")]
    BadAlias,
    #[error("{owner}/{name} already names instrument {existing}; a name never re-points — register under a new name")]
    NameConflict {
        owner: String,
        name: String,
        existing: String,
    },
    #[error("registry io: {0}")]
    Io(#[from] std::io::Error),
    #[error("corrupt registry entry {path}: {message}")]
    Corrupt { path: String, message: String },
}

struct Inner {
    instruments: BTreeMap<ContentHash, Instrument>,
    registrations: Vec<InstrumentRegistration>,
}

/// In-memory view over the on-disk ledger; all writes go through here.
pub struct Registry {
    dir: PathBuf,
    inner: Mutex<Inner>,
}

/// One row of the public listing.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RegistryListing {
    #[serde(flatten)]
    pub registration: InstrumentRegistration,
    pub currency: Currency,
}

impl Registry {
    /// Load (or initialize) a registry rooted at `dir`.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, RegistryError> {
        let dir = dir.into();
        let instrument_dir = dir.join("instruments");
        fs::create_dir_all(&instrument_dir)?;
        let mut instruments = BTreeMap::new();
        for entry in fs::read_dir(&instrument_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path)?;
            let instrument: Instrument =
                serde_json::from_slice(&bytes).map_err(|e| RegistryError::Corrupt {
                    path: path.display().to_string(),
                    message: e.to_string(),
                })?;
            let hash = instrument.content_hash();
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if stem != hash.0 {
                return Err(RegistryError::Corrupt {
                    path: path.display().to_string(),
                    message: format!("file name does not match content hash {}", hash.0),
                });
            }
            instruments.insert(hash, instrument);
        }
        let ledger = dir.join("registrations.jsonl");
        let mut registrations = Vec::new();
        if ledger.exists() {
            for (index, line) in fs::read_to_string(&ledger)?.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let row: InstrumentRegistration =
                    serde_json::from_str(line).map_err(|e| RegistryError::Corrupt {
                        path: format!("{}:{}", ledger.display(), index + 1),
                        message: e.to_string(),
                    })?;
                registrations.push(row);
            }
        }
        Ok(Self {
            dir,
            inner: Mutex::new(Inner {
                instruments,
                registrations,
            }),
        })
    }

    /// Validate, persist, and alias an instrument. Idempotent on identical
    /// content; a conflicting re-use of `(owner, name)` is refused.
    pub fn register(
        &self,
        instrument: Instrument,
        name: &str,
        owner: &AccountId,
    ) -> Result<InstrumentRegistration, RegistryError> {
        if name.is_empty() || name.len() > MAX_NAME_BYTES {
            return Err(RegistryError::BadAlias);
        }
        if owner.0.is_empty() || owner.0.len() > MAX_OWNER_BYTES {
            return Err(RegistryError::BadAlias);
        }
        instrument.validate()?;
        let hash = instrument.content_hash();
        let mut inner = self.inner.lock().expect("registry lock");
        if let Some(existing) = inner
            .registrations
            .iter()
            .find(|r| r.owner == *owner && r.name == name)
        {
            if existing.instrument == hash {
                return Ok(existing.clone());
            }
            return Err(RegistryError::NameConflict {
                owner: owner.0.clone(),
                name: name.to_string(),
                existing: existing.instrument.0.clone(),
            });
        }
        let instrument_path = self
            .dir
            .join("instruments")
            .join(format!("{}.json", hash.0));
        if !instrument_path.exists() {
            let tmp = instrument_path.with_extension("json.tmp");
            fs::write(
                &tmp,
                serde_json::to_vec_pretty(&instrument).expect("serializes"),
            )?;
            fs::rename(&tmp, &instrument_path)?;
        }
        let registration = InstrumentRegistration {
            instrument: hash.clone(),
            name: name.to_string(),
            owner: owner.clone(),
            registered_at: chrono::Utc::now().to_rfc3339(),
        };
        let mut ledger = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("registrations.jsonl"))?;
        let mut line = serde_json::to_vec(&registration).expect("serializes");
        line.push(b'\n');
        ledger.write_all(&line)?;
        inner.instruments.insert(hash, instrument);
        inner.registrations.push(registration.clone());
        Ok(registration)
    }

    /// The full instrument plus every alias pointing at it.
    pub fn get(&self, hash: &str) -> Option<(Instrument, Vec<InstrumentRegistration>)> {
        let inner = self.inner.lock().expect("registry lock");
        let key = ContentHash(hash.to_string());
        let instrument = inner.instruments.get(&key)?.clone();
        let registrations = inner
            .registrations
            .iter()
            .filter(|r| r.instrument == key)
            .cloned()
            .collect();
        Some((instrument, registrations))
    }

    /// Every registration with its instrument's currency.
    pub fn list(&self) -> Vec<RegistryListing> {
        let inner = self.inner.lock().expect("registry lock");
        inner
            .registrations
            .iter()
            .map(|registration| RegistryListing {
                registration: registration.clone(),
                currency: inner
                    .instruments
                    .get(&registration.instrument)
                    .expect("every registration has its instrument")
                    .currency(),
            })
            .collect()
    }

    /// Register the three live instruments under the platform account.
    /// Idempotent across restarts.
    pub fn seed_builtins(&self) -> Result<Vec<InstrumentRegistration>, RegistryError> {
        let owner = AccountId(PLATFORM_ACCOUNT.to_string());
        [
            ("ratio_letter_v1", builtin::ratio_letter_v1()),
            ("ordinal_letter_v1", builtin::ordinal_letter_v1()),
            ("canonical_v2", builtin::canonical_v2()),
        ]
        .into_iter()
        .map(|(name, instrument)| self.register(instrument, name, &owner))
        .collect()
    }
}

/// The live instruments expressed in contract types, template bytes derived
/// from the engine's own renderers.
pub mod builtin {
    use std::collections::BTreeMap;

    use llmsort::prompts::PROMPT_V2;
    use llmsort::seriate::atom::RATIO_LADDER;
    use llmsort::seriate::instrument::ordinal::OrdinalInstrument;
    use llmsort::seriate::instrument::ratio_letter::RatioLetterInstrument;
    use llmsort::seriate::instrument::{Instrument as SeriateRender, RenderedPrompt};
    use llmsort::seriate::ontology::{Attribute, Entity};

    use crate::openpriors::{
        Arity, Decode, Direction, Instrument, Interpretation, LadderRung, OutputSpace,
        ReasoningPin, Signature, TemplateFamily, Turn,
    };

    const REFUSAL: &str = "!";

    /// Render an engine instrument with the contract's slot literals as
    /// bodies, so the returned bytes ARE the template.
    fn render_skeleton(instrument: &dyn SeriateRender) -> RenderedPrompt {
        instrument.render(
            &Attribute::new("{attribute_name}", "{attribute_text}"),
            &Entity::new("{entity_a}"),
            &Entity::new("{entity_b}"),
        )
    }

    fn single_token_turn(system: String, user: String) -> TemplateFamily {
        TemplateFamily {
            turns: vec![Turn {
                system: Some(system),
                user,
                decode: Decode {
                    max_tokens: 1,
                    forbid_verdict: false,
                    reasoning: ReasoningPin::Unpinned,
                    read_logprobs: true,
                },
            }],
        }
    }

    /// The flagship: 52-letter single-token ratio ladder, logprob-native.
    pub fn ratio_letter_v1() -> Instrument {
        let rendered = render_skeleton(&RatioLetterInstrument);
        let mut rungs = BTreeMap::new();
        rungs.insert(
            "A".to_string(),
            LadderRung {
                direction: Direction::Equal,
                ratio: 1.0,
            },
        );
        rungs.insert(
            "a".to_string(),
            LadderRung {
                direction: Direction::Equal,
                ratio: 1.0,
            },
        );
        for (i, ratio) in RATIO_LADDER.iter().enumerate() {
            let upper = ((b'B' + i as u8) as char).to_string();
            let lower = ((b'b' + i as u8) as char).to_string();
            rungs.insert(
                upper,
                LadderRung {
                    direction: Direction::FirstHigher,
                    ratio: *ratio,
                },
            );
            rungs.insert(
                lower,
                LadderRung {
                    direction: Direction::SecondHigher,
                    ratio: *ratio,
                },
            );
        }
        Instrument {
            signature: Signature {
                arity: Arity::Pair,
                output: OutputSpace::SingleToken {
                    alphabet: rendered.answer_alphabet.clone(),
                },
                refusal_tokens: vec![REFUSAL.to_string()],
            },
            template: single_token_turn(rendered.system, rendered.user),
            interpretation: Interpretation::RatioLadder { rungs },
        }
    }

    /// Direction-only single-token read; parity and magnitude censoring ride
    /// the engine's evidence path.
    pub fn ordinal_letter_v1() -> Instrument {
        let rendered = render_skeleton(&OrdinalInstrument);
        Instrument {
            signature: Signature {
                arity: Arity::Pair,
                output: OutputSpace::SingleToken {
                    alphabet: rendered.answer_alphabet.clone(),
                },
                refusal_tokens: vec![REFUSAL.to_string()],
            },
            template: single_token_turn(rendered.system, rendered.user),
            interpretation: Interpretation::OrdinalLetter {
                first_higher: vec!["A".to_string()],
                second_higher: vec!["B".to_string()],
                equal: vec!["=".to_string()],
            },
        }
    }

    /// The sampled-JSON pairwise ratio prompt (the adaptive path's default
    /// where logprobs are not served). Template bytes are the engine's
    /// `PROMPT_V2` verbatim, placeholders included.
    pub fn canonical_v2() -> Instrument {
        Instrument {
            signature: Signature {
                arity: Arity::Pair,
                output: OutputSpace::Json {
                    schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "higher_ranked": { "enum": ["A", "B"] },
                            "ratio": { "type": "number", "minimum": 1.0, "maximum": 26.0 },
                            "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                            "refused": { "type": "boolean" }
                        }
                    }),
                },
                refusal_tokens: Vec::new(),
            },
            template: TemplateFamily {
                turns: vec![Turn {
                    system: Some(PROMPT_V2.system.to_string()),
                    user: PROMPT_V2.user.to_string(),
                    decode: Decode {
                        max_tokens: 256,
                        forbid_verdict: false,
                        reasoning: ReasoningPin::Unpinned,
                        read_logprobs: false,
                    },
                }],
            },
            interpretation: Interpretation::RatioJson {
                higher_pointer: "/higher_ranked".to_string(),
                ratio_pointer: "/ratio".to_string(),
            },
        }
    }
}
