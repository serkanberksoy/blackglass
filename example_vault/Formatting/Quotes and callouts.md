# Quotes and callouts

## Quotes

> A quote starts each line with `>`.
>
> > And a quote in a quote with `> >`.

## Callouts

A quote whose first line is `> [!type] Title` is a callout: a colored box
with an icon. Without a title, the type is its title.

> [!note]
> `[!note]`: a plain remark.

> [!abstract] Abstract
> `[!abstract]`, also `[!summary]`, `[!tldr]`.

> [!info] Info
> `[!info]`.

> [!todo] To do
> `[!todo]`.

> [!tip] Tip
> `[!tip]`, also `[!hint]`, `[!important]`.

> [!success] Success
> `[!success]`, also `[!check]`, `[!done]`.

> [!question] Question
> `[!question]`, also `[!help]`, `[!faq]`.

> [!warning] Warning
> `[!warning]`, also `[!caution]`, `[!attention]`.

> [!failure] Failure
> `[!failure]`, also `[!fail]`, `[!missing]`.

> [!danger] Danger
> `[!danger]`, also `[!error]`.

> [!bug] Bug
> `[!bug]`.

> [!example] Example
> `[!example]`.

> [!quote] Quote
> `[!quote]`, also `[!cite]`.

> [!recipe] Any other name
> `[!recipe]` or any word: a callout in the default color.

## Folding

> [!tip]- Folded: Ctrl+K on this line opens it
> A `-` after the type starts it folded.

> [!info]+ Open, but foldable
> A `+` starts it open; **Ctrl+K** folds it.

## Callouts in callouts

> [!question] Can callouts nest?
> Yes, one `>` more per level:
>
> > [!success] Like this
> > With *formatting*, `code` and [[Lists|links]] inside.
