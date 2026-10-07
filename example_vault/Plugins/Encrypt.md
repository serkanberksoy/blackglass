# Encrypt

The **Encrypt** plugin hides part of a note behind a password. What it
writes is the format of the Meld Encrypt plugin, so other apps with that
plugin can read it too.

## Decrypt

The line below is encrypted; it shows as 🔐 and its hint. Put the cursor on
it, run **Decrypt** from the palette (**Ctrl+P**) and type the password,
`blackglass`:

The secret: 🔐β 💡the app's name💡XyfKzWmZtcvwo7NeC5KCo9JRqvrzE7+9ouK/SkGpYLCamBk8h9GCBynVxkQS+vw/BCiy34OfXt/JwB6p9TLJZoOvT4o3e55IdPlZ4CU1xV5AK2ZArrFAb74= 🔐

The text is shown in a window: **Copy** puts it on the clipboard,
**Decrypt in place** puts it back in the note as plain text, **Close**
leaves it encrypted.

## Encrypt

Select some text below (Shift and the arrows), run **Encrypt selection**,
type a password twice and, if you like, a hint (it's not encrypted, so
anyone can read it). With nothing selected, the window asks for the text.

My bank PIN is 1234.

"When reading" chooses whether the 🔐 shows, or the encrypted text is
hidden in a `%%` comment.

## Passwords

Settings → Encrypt: a password can be remembered while blackglass runs (for
the note, its folder or the whole vault) and forgotten after some minutes;
it's never saved. **Forget passwords** forgets them at once. A forgotten
password can't be recovered: the text stays locked.
