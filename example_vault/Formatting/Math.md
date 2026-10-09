# Math

LaTeX between dollar signs reads as Unicode, as near as text gets. Move
the cursor onto a line to see what's written.

## Inline

- Energy: $e = mc^2$
- Greek letters: $\alpha + \beta = \gamma$, $\Delta x \to 0$
- Sets: $x \in \mathbb{R}$, $A \cup B \subseteq C$, $\forall n \in \mathbb{N}$
- Roots and fractions: $\sqrt{2} \approx 1.414$, $\frac{1}{2} + \frac{a+b}{c}$
- Subscripts: $x_1, x_2, \ldots, x_n$
- Not math: prices like $5 and $10.

## Blocks

$$
\sum_{i=1}^{n} i = \frac{n(n+1)}{2}
$$

$$ e^{i\pi} + 1 = 0 $$

$$
\int_0^1 x^2 \, dx = \frac{1}{3}
$$

Superscripts and subscripts use Unicode's where it has them all (`x²`,
`xₙ`); otherwise they're written `^(…)` and `_(…)`. A command it doesn't
know stays as written.
