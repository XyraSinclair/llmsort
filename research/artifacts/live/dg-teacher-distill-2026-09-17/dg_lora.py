"""Hand-rolled LoRA for DiffusionGemma's decoder stack (peft is not in the host venv)."""
import math, torch
from torch import nn

TARGETS = ("self_attn.q_proj", "self_attn.k_proj", "self_attn.v_proj", "self_attn.o_proj", "mlp.gate_proj", "mlp.up_proj", "mlp.down_proj")


class LoRALinear(nn.Module):
    def __init__(self, base, r, alpha):
        super().__init__()
        self.base = base
        self.A = nn.Parameter(torch.empty(r, base.in_features, dtype=torch.float32))
        self.B = nn.Parameter(torch.zeros(base.out_features, r, dtype=torch.float32))
        nn.init.kaiming_uniform_(self.A, a=math.sqrt(5))
        self.scale = alpha / r

    def forward(self, x):
        y = self.base(x)
        return y + ((x.float() @ self.A.t()) @ self.B.t()).to(y.dtype) * self.scale


def apply(model, r=16, alpha=32, device="cuda:0"):
    """Wrap the decoder's attention and dense-MLP projections. Encoder and experts untouched."""
    wrapped = {}
    for layer in model.model.decoder.layers:
        for name in TARGETS:
            parent_name, attr = name.split(".")
            parent = getattr(layer, parent_name); base = getattr(parent, attr)
            if base is None: continue                      # global layers have no v_proj
            lora = LoRALinear(base, r, alpha).to(device)
            setattr(parent, attr, lora); wrapped[f"{layer.layer_idx}.{name}"] = lora
    return wrapped


def parameters(wrapped):
    return [p for l in wrapped.values() for p in (l.A, l.B)]


def state(wrapped):
    return {k: {"A": l.A.detach().cpu(), "B": l.B.detach().cpu()} for k, l in wrapped.items()}


def load(wrapped, path):
    st = torch.load(path, map_location="cpu")
    assert set(st) == set(wrapped), (len(st), len(wrapped))
    for k, l in wrapped.items():
        l.A.data.copy_(st[k]["A"].to(l.A.device)); l.B.data.copy_(st[k]["B"].to(l.B.device))
