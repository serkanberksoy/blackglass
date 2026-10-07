# Encrypt: what blackglass has and what's missing

A comparison of blackglass's Encrypt plugin (W-134) with the community
plugin [Meld Encrypt](https://github.com/meld-cp/obsidian-encrypt) by
meld-cp (its source, read 2026-10-07). IDs are `EN-xx`.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## 1. In-place encryption (a selection)

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| EN-01 | The original's format: `%%🔐β 💡hint💡<base64> 🔐%%`, or `🔐β … 🔐` with the marker shown when reading; AES-256-GCM, a 16-byte nonce and salt, PBKDF2-SHA-512 with 210,000 rounds | ✅ | `encrypt::crypto::tests::texts_are_encrypted_as_meld_encrypt_writes_them`; checked both ways against the original's WebCrypto code in Node |
| EN-02 | Older texts read: α (PBKDF2-SHA-256, 1,000 rounds, a fixed salt) and the first version (the password's SHA-256, a fixed nonce) | ✅ | `encrypt::crypto::tests::meld_encrypts_texts_are_read` (vectors made by the original's code) |
| EN-03 | "Encrypt selection": password, confirmation, hint, marker shown or hidden; the selection replaced (one undo step) | ✅ | `workspace::a_selection_is_encrypted_and_decrypted_with_a_password`; passwords shown as dots |
| EN-04 | Nothing selected: the text typed in the window (or the whole line, a setting); refused inside encrypted text | ✅ | `workspace::meld_encrypts_text_is_decrypted_with_its_hint` |
| EN-05 | "Decrypt" at the cursor: the hint shown, the password asked; the text shown with Copy and Decrypt in place | ✅ | Both workspace tests; a wrong password says so |
| EN-06 | Remembered passwords: for the note, its folder or the vault, forgotten after N minutes (0: on quit); never saved | ✅ | Tried first when decrypting, filled in when encrypting; "Forget passwords" |
| EN-07 | Encrypted text shown as its marker and hint (the original's reading view) | ✅ | 🔐 (`[locked]` without Unicode) and the hint where the cursor isn't (mdedit's rendered spans) |
| EN-08 | Remember passwords by an external key file (the original's "external file" level) | ⬜ | |
| EN-09 | Clicking the marker to decrypt (the original's reading view) | ⬜ | Needs clicks on rendered spans from mdedit (with W-137) |

## 2. Not planned

| ID | Feature | Now | Notes |
|----|---------|-----|-------|
| EN-20 | Whole-note encryption: `.mdenc` notes, converting a note and back, the older `.encrypted` files | ✗ | Not wanted (decided 2026-10-07): selections only |
| EN-30 | Ribbon icons | ✗ | No ribbon; the commands are in the palette and can have keys |
