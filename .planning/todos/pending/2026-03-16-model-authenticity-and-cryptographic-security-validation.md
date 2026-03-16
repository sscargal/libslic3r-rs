---
created: 2026-03-16T19:32:48.250Z
title: Model authenticity and cryptographic security validation
area: fileio
files:
  - crates/slicecore-fileio/src/threemf.rs
  - crates/slicecore-fileio/src/stl.rs
  - crates/slicecore-fileio/src/lib.rs
---

## Problem

3D model files (STL, 3MF, OBJ) have zero authenticity guarantees. Anyone can:

- **Distribute tampered models**: A safety-critical part (drone propeller, medical device bracket) can be modified maliciously without detection
- **Claim false authorship**: No way to prove who designed a model
- **Violate licensing**: Paid models on Printables/Thangs/MyMiniFactory get pirated with no provenance trail
- **Insert defects**: Subtle mesh modifications (thinned walls, weakened internal geometry) are invisible to the naked eye but cause structural failure
- **Counterfeit parts**: In manufacturing, there's no way to verify a printed part came from an authorized model file

This is especially critical for:
- **Aerospace/defense**: FAA/DoD require part provenance and chain of custody
- **Medical devices**: FDA requires traceability of design files
- **Print farms/SaaS**: Operators need to verify they're printing the authorized version
- **Designers/creators**: IP protection and attribution

## Solution

### Core: Asymmetric cryptographic signing (SSH/HTTPS model)

Model creators sign their files with a private key. Anyone can verify authenticity using the creator's public key. The signature is one-way — it cannot be removed, modified, or replaced without detection.

### How it works

```
Creator side:
  Model file → SHA-256 hash → Sign with private key → Signature

Verifier side:
  Model file → SHA-256 hash → Verify against signature + public key → ✓ or ✗
```

### Signature formats

#### Option A: Detached signature file
```
model.stl           ← Original model (unchanged)
model.stl.sig       ← Detached Ed25519 signature
model.stl.pubkey    ← Creator's public key (or key ID for lookup)
```
- Pros: Works with any file format, doesn't modify the model
- Cons: Signature file can be separated from model

#### Option B: Embedded in 3MF metadata
3MF is a ZIP archive — signatures can be embedded as metadata:
```xml
<!-- Inside 3MF [Content_Types].xml or custom extension -->
<Signature>
  <Algorithm>Ed25519</Algorithm>
  <Creator>designer@example.com</Creator>
  <KeyFingerprint>SHA256:abc123...</KeyFingerprint>
  <SignedHash>base64_encoded_signature...</SignedHash>
  <Timestamp>2026-03-16T19:00:00Z</Timestamp>
  <SignedFields>mesh_data,metadata,build_items</SignedFields>
</Signature>
```
- Pros: Self-contained, travels with the model
- Cons: 3MF-specific, requires format extension

#### Option C: Mesh watermarking (steganographic)
Embed signature data directly in the mesh geometry as imperceptible vertex perturbations:
- Modify vertex positions by sub-micron amounts that encode signature bits
- Survives format conversion (STL → 3MF → OBJ) since it's in the geometry itself
- Cannot be stripped without destroying the signature (one-way)
- Detectable by the slicer even after format conversion
- Limited capacity: ~100-500 bits depending on mesh complexity

### Recommended: Layered approach (all three)

| Layer | Method | Purpose | Survives format conversion? |
|-------|--------|---------|---------------------------|
| 1 | Detached `.sig` file | Full cryptographic proof | No — file can be separated |
| 2 | 3MF embedded metadata | Self-contained verification | Only in 3MF |
| 3 | Mesh watermark | Tamper-evident, survives conversion | Yes — in the geometry |

### Key management

#### Creator workflow
```bash
# Generate signing keypair (once)
slicecore key generate --name "Designer Name" --email "designer@example.com"
# → Private key: ~/.config/slicecore/keys/private.ed25519
# → Public key:  ~/.config/slicecore/keys/public.ed25519

# Sign a model
slicecore sign model.3mf --key ~/.config/slicecore/keys/private.ed25519
# → Embeds signature in 3MF metadata
# → Creates model.3mf.sig (detached signature)
# → Optionally applies mesh watermark (--watermark flag)

# Publish public key
slicecore key publish --to keyserver.slicecore.dev
# Or: export as file, share via website, embed in Printables profile
```

#### Verifier workflow
```bash
# Verify a model before slicing
slicecore verify model.3mf
# ✓ Signed by: Designer Name <designer@example.com>
# ✓ Key fingerprint: SHA256:abc123...
# ✓ Signed: 2026-03-16T19:00:00Z
# ✓ Mesh integrity: hash matches (file unmodified)
# ✓ Watermark: present and valid

# Verify against a known public key
slicecore verify model.3mf --key designer-publickey.ed25519

# Automatic verification during slicing
slicecore slice model.3mf --require-signed
# → Refuses to slice unsigned or tampered models

# Trust on first use (TOFU) — like SSH
slicecore slice model.3mf
# ⚠ Unknown signer: Designer Name (SHA256:abc123...)
# Trust this key? [y/N/always]
```

### Key infrastructure options

| Approach | How it works | Trust model |
|----------|-------------|-------------|
| **Self-signed** | Creator generates keypair, shares public key manually | Direct trust (like SSH) |
| **TOFU** | Slicer remembers public keys on first encounter | Trust on first use (SSH-like) |
| **Keyserver** | Central directory of public keys (like PGP keyservers) | Federated trust |
| **PKI/CA** | Certificate authority signs creator keys | Hierarchical trust (HTTPS-like) |
| **Web of Trust** | Creators vouch for each other's keys | Decentralized (PGP-like) |
| **Platform integration** | Printables/Thangs embed signatures, platform is the CA | Platform trust |

**Recommended MVP**: Self-signed + TOFU (simplest, no infrastructure needed). Add keyserver later.

### Innovation: Blockchain-anchored timestamps

For regulatory/certification use cases, anchor signature timestamps to a blockchain (Bitcoin, Ethereum) or RFC 3161 timestamping authority:
- Proves the model existed and was signed at a specific time
- Prevents backdating signatures
- Immutable audit trail for supply chain compliance

### Innovation: Differential signing

Sign not just the model, but specific properties:
```
Signature covers:
  ✓ Mesh geometry (vertex positions, faces)
  ✓ Mesh topology (connectivity)
  ✓ Critical dimensions (bounding box, volume)
  ✗ Non-critical metadata (color, texture coordinates)
  ✗ File format specifics (allows STL↔3MF conversion)
```
This allows format conversion while preserving the signature — the signature is over the *geometry*, not the *file*.

### Innovation: Graduated trust levels

```toml
[security]
# What to do with unsigned models
unsigned_models = "warn"    # "allow" | "warn" | "block"

# What to do with tampered models (signature doesn't match)
tampered_models = "block"   # "warn" | "block"

# Trusted signers (auto-accept)
trusted_keys = [
    "SHA256:abc123...",     # Designer A
    "SHA256:def456...",     # Company B
]

# Required for specific operations
require_signature_for = ["production_slice", "farm_submit"]
```

### Innovation: Print-to-part traceability

Extend signing to the full pipeline:
```
Model (signed by designer)
  → Slice config (signed by operator)
    → G-code (signed by slicer + config hash)
      → Print job (signed by farm manager)
        → Physical part (QR code links to full chain)
```

Each step signs its output, including a reference to the previous step's signature. The physical part's QR code links to the complete chain of custody — from designer to printed part.

## Cryptographic choices

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Signature algorithm | **Ed25519** | Fast, small signatures (64 bytes), widely supported, no patents |
| Hash function | **SHA-256** | Industry standard, hardware-accelerated |
| Key format | **SSH-compatible** | Users can reuse existing SSH keys |
| Encoding | **Base64** | Human-readable, embeddable in metadata |
| Rust crate | **ed25519-dalek** or **ring** | Pure Rust, audited, no C dependencies |

## Dependencies

- **3MF support** (Phase 22/24): For embedded metadata signatures
- **STL/OBJ read** (existing): For computing mesh hashes
- **Model library** (todo): Signatures should integrate with library metadata
- **ed25519-dalek / ring**: Cryptographic primitives (pure Rust)

## Phased implementation

1. **Phase A**: Detached signature files (`.sig`) with Ed25519 — works with any format
2. **Phase B**: 3MF embedded signatures — self-contained signed models
3. **Phase C**: TOFU key management and `--require-signed` flag
4. **Phase D**: Mesh watermarking (steganographic signature in vertex data)
5. **Phase E**: Keyserver and platform integration (Printables, Thangs)
6. **Phase F**: Print-to-part traceability chain
