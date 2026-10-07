# Headings and sections

A heading starts a **section**: everything under it up to the next heading
of the same level or higher. Sections are what you fold, jump to, link to
and embed. **Alt+O** lists this note's headings (type to filter, Enter
jumps); **Ctrl+K** on a heading folds its section.

## Six levels

One `#` to six, then a space:

# Level one
## Level two
### Level three
#### Level four
##### Level five
###### Level six

`#tag` without a space is a tag, not a heading.

## Underlined headings

A line underlined with `===` is a level one heading, with `---` a level two:

Underlined, level one
=====================

Underlined, level two
---------------------

## Folding

Put the cursor on the heading below and press **Ctrl+K**: its section
folds, up to "Rules between sections".

### A section to fold

These lines are inside the section.

#### A smaller one inside it

Folding the section above hides this one too; folding this one keeps the
rest of "A section to fold" open.

## Rules between sections

Three dashes, stars or underscores on a line of their own draw a rule:

---

***

___

(Right after a line of text, `---` underlines it as a heading instead;
leave a blank line before a rule.)

## Linking to sections

- In this note: [[#Six levels]], [[#Folding|the folding section]].
- In another note: [[Text styles#Escapes]], [[Lists#Task states]].
- Type `[[Text styles#` to have the headings suggested.

A section can be embedded, shown here as if it were written here:

![[Tables#Alignment]]
