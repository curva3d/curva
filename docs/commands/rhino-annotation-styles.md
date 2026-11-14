# Rhino 8's annotation styles, for Curva

Written on 14 November 2026 from Rhino 8's help page `documentproperties/annotation.htm` ("Annotation
Styles", dated 07-Nov-2026), which the owner provided as a PDF after the direct fetches of
`commands/dimstyle.htm` and `options/annotation_styles.htm` were refused. It is the source for Curva's
annotation styles (spec `docs/specs/annotation.md` §1): names are Rhino's. The **In Curva** column says
when each setting arrives.

## The document's annotation settings

| Rhino | What it does | In Curva |
|---|---|---|
| Current | The style new annotation takes | 0.4.3, Annotation Styles panel |
| Annotation Styles (list) | The styles in the model | 0.4.3 |
| New | A new style from an existing or built-in one | 0.4.3 (as Copy) |
| Match | Copies all settings from one style to another | with dimensions |
| Import | Styles from a `.3dm` file | with the `.3dm` round trip |
| Edit | Opens a style's settings | 0.4.3 |
| Delete | Deletes a style; **Rhino also deletes the annotations using it** | 0.4.3, only for unused styles, as the approved spec says |
| Enable model space scaling | Annotation in model views is multiplied by the style's Model space scale | with dimensions |
| Enable layout space scaling | Annotation in details keeps one size on the sheet whatever the detail's zoom | with dimensions |

Rhino's recommended workflow: set the text height for layouts in the style; turn on model space
scaling; set the model space scale (usually the inverse of the print scale).

## One style's settings

| Section | Rhino's settings | In Curva |
|---|---|---|
| Header | Style name; Model space scale (multiplies text height, extension line extension and offset, arrow length); Adjusted text height (read only); Scale all sizes | name now; scales with dimensions |
| Font | Font (missing fonts shown "(not installed)" with a substitute); B (bold; italic and underline are per-object properties); Height (text height); Text gap (between text and dimension line, and the mask margin); Text reads forward when viewed from behind | font and height now; the rest with dimensions |
| Mask | None, Background, Solid Color; Mask Color; Mask margins; Frame around text: No frame, Rectangular frame, Capsule frame | after dimensions |
| Text | Horizontal alignment (Left, Center, Right); vertical alignment (Top, Middle, Bottom); Fit text (Auto, Inside, Right, Left, HintLeft/HintRight); Horizontal to view | horizontal alignment per text now; the rest with dimensions |
| Dimensions | Dimension text and Radial dimension text (Horizontal to view, In line, Above line, Aligned, Horizontal); Dimension line extension; Extension line extension; Extension line offset; Fixed length extension; Suppress extension 1/2; Baseline spacing; Centermark size; Centermark style (None, Mark, Mark and Lines); Decimal separator (period, comma) | with dimensions |
| Arrows | Arrowhead 1/2 (Arrow, Dot, Tick, Short arrow, Open arrow, Rectangle, Thin arrow, Thinner arrow, User arrow from a block, No arrow); Arrow size; Fit arrow (Auto, Inside, Outside); Leader arrowhead; Leader arrow size; Draw dimension line between extension lines if arrows are outside | with dimensions and leaders |
| Length units | Unit - Format; Length factor; Linear resolution; Round off; Prefix / Suffix (shown when the text contains `<>`); Zero suppression (none, leading, trailing, both) | with dimensions |
| Alternate units | Use alternate units, with the same settings as Length units; Alternate units below; Fraction stacking format (No stack, Stack horizontal); Stack height scale (%) | with dimensions |
| Angular units | Decimal degrees, Degrees minutes seconds, Radians; Angular resolution; Round off; Zero suppression | with dimensions |
| Leaders | Curve type (None, Polyline, Spline); Landing; leader text Horizontal to view; justification (Left, Center, Right, Auto); vertical alignment (Top, Middle of first line, Bottom of first line, Middle, Middle of last line, Bottom, Underlined); Text alignment (Horizontal, Aligned) | with leaders |
| Tolerance | Tolerance style (No tolerance, Symmetrical, Deviation, Limits); Resolution; Alt resolution; Upper value; Lower value; Text height scale (%) | with dimensions |

## Related commands

| Rhino | What it does | In Curva |
|---|---|---|
| SetCurrentAnnotationStyle | Sets the current style | 0.4.3 |
| DupAnnotationStyle | Copies a style: the style to copy, the new name, and `Set current` | 0.4.3 |
| ImportAnnotationStyles | Styles from a `.3dm` file | with the `.3dm` round trip |
