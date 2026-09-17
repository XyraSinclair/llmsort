"""Masked-diffusion readers for the setwise bench: Nemotron-Labs-Diffusion (block over a causal KV
cache; also runs AR) and LLaDA2.2 (block-causal full forward). Same result dict as dg_setwise.Reader."""
import itertools, math, os, random, time
import numpy as np
import torch
from dg_setwise import SLOTS, CACHE

class MaskedReader:
    def __init__(self, tok, model, k, reads, seed, mask_id):
        self.tok, self.model, self.k, self.reads, self.mask_id = tok, model, k, reads, mask_id
        self.rng = random.Random(seed)
        one = lambda s: (lambda ids: ids[0] if len(ids) == 1 else None)(tok(s, add_special_tokens=False).input_ids)
        self.bare = [one(SLOTS[i]) for i in range(k)]
        self.spaced = [one(" " + SLOTS[i]) for i in range(k)]
        assert None not in self.bare and None not in self.spaced, ("slot letters must be single tokens", self.bare, self.spaced)
        self.perms = np.array(list(itertools.permutations(range(k))))
        self.prefix = []          # tokens the model always opens its turn with (none by default)
        self.chat_kwargs = {}     # extra apply_chat_template kwargs (e.g. enable_thinking=False)

    def template(self, m):
        letters = " ".join(SLOTS[: self.k])
        text = letters if m == 1 else "\n".join(f"{i+1}: {letters}" for i in range(m))
        ids = self.prefix + self.tok(text, add_special_tokens=False).input_ids
        n0 = len(self.prefix)
        holes = [(p, self.bare if t in self.bare else self.spaced) for p, t in enumerate(ids) if p >= n0 and (t in self.bare or t in self.spaced)]
        assert len(holes) == m * self.k, (len(holes), text, ids)
        for j, (p, labels) in enumerate(holes): assert ids[p] == labels[j % self.k]
        return text, ids, [holes[i * self.k:(i + 1) * self.k] for i in range(m)]

    def prompt_ids(self, system, user):
        msgs = [{"role": "system", "content": system}, {"role": "user", "content": user}]
        prompt = self.tok.apply_chat_template(msgs, tokenize=False, add_generation_prompt=True, **self.chat_kwargs)
        return self.tok(prompt, add_special_tokens=False).input_ids

    @torch.no_grad()
    def greedy_fill(self, pids, n, bl, eos):
        """Free generation through block_logits: blocks of bl all-masked, fill the most confident hole
        one token per pass (low-confidence remasking, greedy). Stops at eos. Format check only."""
        self.prefill(pids); out = []
        for b0 in range(0, n, bl):
            block = [self.mask_id] * bl
            for _ in range(bl):
                lg = self.block_logits(out + block)[len(out):]
                p = torch.softmax(lg, -1); conf, tokv = p.max(-1)
                holes = [i for i, t in enumerate(block) if t == self.mask_id]
                i = max(holes, key=lambda i: float(conf[i])); block[i] = int(tokv[i])
            out += block
            if eos in block: break
        return out

    # model-specific: prefill(pids) prepares state; block_logits(block_ids) -> [len(block), V] float logits
    def prefill(self, pids): raise NotImplementedError
    def block_logits(self, block): raise NotImplementedError

    def _slot_pmf(self, logits, p, labels, used):
        row = logits[p]
        lse = torch.logsumexp(row, -1)
        ll = row[torch.tensor(labels, device=row.device)]
        mass = float(torch.exp(ll - lse).sum())
        if used is not None:
            ll = ll.masked_fill(used, float("-inf"))
        pr = torch.softmax(ll, -1)
        return pr, mass

    @torch.no_grad()
    def read(self, system, user, m):
        """One forward, every slot masked: mean-field slot marginals, best permutation."""
        text, tids, lines = self.template(m)
        pids = self.prompt_ids(system, user)
        self.prefill(pids)
        block = list(tids)
        for line in lines:
            for p, _ in line: block[p] = self.mask_id
        logits = self.block_logits(block)
        parsed, holes_out, mats, perm_ok = [], [], [], []
        for line in lines:
            rows, masses = [], []
            for p, labels in line:
                pr, mass = self._slot_pmf(logits, p, labels, None); rows.append(pr); masses.append(mass)
            mean = torch.stack(rows)                                   # k x k
            top = mean.argmax(-1)
            ent = -(mean * torch.log(mean.clamp_min(1e-30))).sum(-1)
            M = torch.log(mean.clamp_min(1e-30)).cpu().numpy()
            best = self.perms[M[np.arange(self.k), self.perms].sum(1).argmax()]
            parsed.append([int(x) for x in best]); perm_ok.append(len(set(top.tolist())) == self.k)
            mats.append(np.round(mean.cpu().numpy(), 4).tolist())
            holes_out.append([{"label_mass": masses[j], "entropy": float(ent[j]), "entropy_first_read": float(ent[j]), "agreement": 1.0, "stderr": 0.0, "p_top": float(mean[j].max())} for j in range(self.k)])
        return {"parsed": parsed, "holes": holes_out, "matrix": mats, "argmax_is_perm": perm_ok, "input_tokens": len(pids), "template": text}

    @torch.no_grad()
    def read_clamp(self, system, user, m):
        """Sequential clamping: stage s fixes slots < s, masks slots >= s, reads slot s over unused letters."""
        text, tids, lines = self.template(m)
        pids = self.prompt_ids(system, user)
        self.prefill(pids)
        k = self.k
        chosen = [[] for _ in lines]; stages = [[] for _ in lines]; holes_out = [[] for _ in lines]
        for s in range(k):
            block = list(tids)
            for li, line in enumerate(lines):
                for j, (p, labels) in enumerate(line):
                    block[p] = labels[chosen[li][j]] if j < s else self.mask_id
            logits = self.block_logits(block)
            for li, line in enumerate(lines):
                p, labels = line[s]
                used = torch.zeros(k, dtype=torch.bool, device=logits.device); used[chosen[li]] = True
                pr, mass = self._slot_pmf(logits, p, labels, used)
                top = int(pr.argmax())
                chosen[li].append(top); stages[li].append(np.round(pr.cpu().numpy(), 4).tolist())
                holes_out[li].append({"label_mass": mass, "entropy": float(-(pr * torch.log(pr.clamp_min(1e-30))).sum()), "agreement": 1.0, "stderr": 0.0, "p_top": float(pr[top]), "remaining": k - s})
        return {"parsed": [list(c) for c in chosen], "holes": holes_out, "matrix": stages, "argmax_is_perm": [True] * m, "input_tokens": len(pids), "template": text}


class NemotronReader(MaskedReader):
    """nvidia/Nemotron-Labs-Diffusion-*: causal prefill into a KV cache, then a bidirectional masked block
    that attends to the cache (the model's own generate() regime). `ar=True` reads the same template
    autoregressively instead (next-token logits with the chosen prefix), for an in-model AR control."""
    def __init__(self, tok, model, k, reads, seed, ar=False):
        super().__init__(tok, model, k, reads, seed, model.config.mask_token_id)
        self.ar = ar

    def _set_dlm(self, val):
        for layer in self.model.encoder.layers:
            if hasattr(layer.self_attn, "diffusion_lm"): layer.self_attn.diffusion_lm = val

    def prefill(self, pids):
        dev = self.model.device
        self._set_dlm(False)
        out = self.model(torch.tensor([pids], device=dev), use_cache=True, use_causal_mask=True)
        self.cache = out.past_key_values
        self.last_logit = out.logits[0, -1].float()
        self._set_dlm(not self.ar)

    def block_logits(self, block):
        dev = self.model.device
        if not self.ar:
            return self.model(torch.tensor([block], device=dev), past_key_values=self.cache, use_cache=False).logits[0].float()
        # AR: logits[p] must predict token p of the block given tokens < p; masks after the read position are irrelevant.
        # One causal pass over the block gives next-token logits at every position; shift by one.
        out = self.model(torch.tensor([block], device=dev), past_key_values=self.cache, use_cache=False, use_causal_mask=True).logits[0].float()
        return torch.cat([self.last_logit.unsqueeze(0), out[:-1]], 0)

    @torch.no_grad()
    def read_clamp(self, system, user, m):
        if not self.ar: return super().read_clamp(system, user, m)
        # AR clamp = constrained greedy decoding; masks after the read slot would poison the causal prefix only
        # if they precede it, and they never do. One pass per stage, reading the slot's next-token PMF.
        return super().read_clamp(system, user, m)


class LLaDA2Reader(MaskedReader):
    """inclusionAI/LLaDA2.x: full forward over prompt + template with the block-causal (32) attention
    mask its generate() uses; no KV cache. Each stage re-encodes the prompt."""
    def __init__(self, tok, model, k, reads, seed, block_length=32, mask_id=156895):
        super().__init__(tok, model, k, reads, seed, mask_id)
        self.bl = block_length

    def prefill(self, pids):
        self.pids = pids

    def block_logits(self, block):
        dev = self.model.device; bl = self.bl
        x = self.pids + block
        total = (len(x) + bl - 1) // bl * bl
        x = x + [self.mask_id] * (total - len(x))
        nb = total // bl
        bm = torch.tril(torch.ones(nb, nb, device=dev)).repeat_interleave(bl, 0).repeat_interleave(bl, 1)
        attn = bm.log().to(torch.bfloat16)[None, None]
        pos = torch.arange(total, device=dev)[None]
        out = self.model(torch.tensor([x], device=dev), attention_mask=attn, position_ids=pos).logits[0]
        return out[len(self.pids):len(self.pids) + len(block)].float()


def load_nemotron(name, device):
    from transformers import AutoTokenizer, AutoModel
    t0 = time.time()
    tok = AutoTokenizer.from_pretrained(name, cache_dir=CACHE, trust_remote_code=True)
    model = AutoModel.from_pretrained(name, cache_dir=CACHE, trust_remote_code=True, dtype=torch.bfloat16).to(device).eval()
    print(f"{name}: {torch.cuda.memory_allocated()/2**30:.1f} GiB in {time.time()-t0:.0f}s", flush=True)
    return tok, model

def load_llada2(name, device):
    from transformers import AutoTokenizer, AutoModelForCausalLM
    t0 = time.time()
    tok = AutoTokenizer.from_pretrained(name, cache_dir=CACHE, trust_remote_code=True)
    model = AutoModelForCausalLM.from_pretrained(name, cache_dir=CACHE, trust_remote_code=True, dtype=torch.bfloat16).to(device).eval()
    print(f"{name}: {torch.cuda.memory_allocated()/2**30:.1f} GiB in {time.time()-t0:.0f}s", flush=True)
    return tok, model


class SDARReader(MaskedReader):
    """JetLM/SDAR-*-Chat (Qwen3-derived block diffusion, block 4). The repo's modeling file targets
    transformers 4.5x; the weights are byte-for-byte Qwen3 keys, so this runs stock Qwen3ForCausalLM
    with the reference generate.py's attention: block-causal tril over blocks of 4 counted from
    position 0 (bidirectional inside a block), position_ids = arange. Passed as a custom 4D bool mask.
    The prompt is prefilled once into a DynamicCache and cropped back after every stage pass."""
    def __init__(self, tok, model, k, reads, seed, block_length=4):
        super().__init__(tok, model, k, reads, seed, tok(tok.mask_token, add_special_tokens=False).input_ids[0])
        self.bl = block_length   # plain `assistant\n` tail: the Chat SFT has no empty think block (with one it emits eos at .68)

    def _mask(self, q0, q1, kv1):
        dev = self.model.device; bl = self.bl
        q = torch.arange(q0, q1, device=dev)[:, None] // bl
        kv = torch.arange(0, kv1, device=dev)[None, :] // bl
        return (kv <= q)[None, None]

    def prefill(self, pids):
        from transformers import DynamicCache
        dev = self.model.device; P = len(pids)
        self.cache = DynamicCache()
        self.model(input_ids=torch.tensor([pids], device=dev), attention_mask=self._mask(0, P, P),
                   position_ids=torch.arange(P, device=dev)[None], cache_position=torch.arange(P, device=dev),
                   past_key_values=self.cache, use_cache=True)
        self.P = P

    def block_logits(self, block):
        dev = self.model.device; P, B = self.P, len(block)
        out = self.model(input_ids=torch.tensor([block], device=dev), attention_mask=self._mask(P, P + B, P + B),
                         position_ids=torch.arange(P, P + B, device=dev)[None], cache_position=torch.arange(P, P + B, device=dev),
                         past_key_values=self.cache, use_cache=True).logits[0]
        self.cache.crop(P)
        return out.float()


class DreamReader(MaskedReader):
    """Dream-org/Dream-v0-Instruct-7B: fully bidirectional MDLM from Qwen2.5-7B, run on stock
    Qwen2ForCausalLM with an all-True 4D mask (the repo's modeling file targets transformers 4.46).
    Its generation loop shifts logits one position left (the output at i-1 predicts token i). No cache,
    full recompute per stage."""
    def __init__(self, tok, model, k, reads, seed, mask_id=151666):
        super().__init__(tok, model, k, reads, seed, mask_id)

    def prefill(self, pids):
        self.pids = pids

    def block_logits(self, block):
        dev = self.model.device
        x = torch.tensor([self.pids + block], device=dev); L = x.shape[1]
        attn = torch.ones(1, 1, L, L, dtype=torch.bool, device=dev)
        out = self.model(input_ids=x, attention_mask=attn, position_ids=torch.arange(L, device=dev)[None], use_cache=False).logits[0]
        n0 = len(self.pids)
        return out[n0 - 1:n0 - 1 + len(block)].float()


def _load_stock(name, device, Config, Model):
    """Weights of a Qwen-derived diffusion checkpoint into the stock transformers class; only the
    config keys the stock class knows survive, rope theta re-homed under rope_parameters."""
    import json, glob
    t0 = time.time()
    snap = glob.glob(os.path.join(CACHE, "models--" + name.replace("/", "--"), "snapshots", "*"))[0]
    cfg = json.load(open(os.path.join(snap, "config.json")))
    keep = {k: v for k, v in cfg.items() if k in Config().to_dict() and k not in ("architectures", "model_type", "attn_implementation")}
    keep["rope_parameters"] = {"rope_type": "default", "rope_theta": cfg["rope_theta"]}
    from transformers import Qwen2TokenizerFast
    tok = Qwen2TokenizerFast.from_pretrained(snap)   # both repos ship Qwen2 vocab/merges + chat_template; skips their custom tokenizer classes
    model, info = Model.from_pretrained(snap, config=Config(**keep), dtype=torch.bfloat16, output_loading_info=True)
    assert not info["missing_keys"] and not info["unexpected_keys"] and not info["mismatched_keys"], info
    model = model.to(device).eval()
    print(f"{name}: {torch.cuda.memory_allocated()/2**30:.1f} GiB in {time.time()-t0:.0f}s (stock {Model.__name__}, rope {model.config.rope_parameters})", flush=True)
    return tok, model

def load_sdar(name, device):
    from transformers import Qwen3Config, Qwen3ForCausalLM
    return _load_stock(name, device, Qwen3Config, Qwen3ForCausalLM)

def load_dream(name, device):
    from transformers import Qwen2Config, Qwen2ForCausalLM
    return _load_stock(name, device, Qwen2Config, Qwen2ForCausalLM)
