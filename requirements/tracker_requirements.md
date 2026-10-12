# Tracker: what blackglass has and what's missing

A comparison of blackglass's Tracker plugin (W-149) with the community
plugin [Tracker](https://github.com/pyrochlore/obsidian-tracker) by
pyrochlore, against its documentation
([input parameters](https://github.com/pyrochlore/obsidian-tracker/blob/master/docs/InputParameters.md),
[target evaluation](https://github.com/pyrochlore/obsidian-tracker/blob/master/docs/TargetEvaluation.md),
[expressions](https://github.com/pyrochlore/obsidian-tracker/blob/master/docs/Expressions.md);
read 2026-10-13). IDs are `TR-xx`.

A ```` ```tracker ```` block is YAML with the original's parameter names,
so blocks written for the original work as they are. Charts are drawn
with text: lines in braille, bars in eighth blocks, a pie in half blocks.
Tests are in `tracker::params::tests` (`params::`), `tracker::collect::tests`
(`collect::`), `tracker::expr::tests` (`expr::`), `tracker::chart::tests`
(`chart::`), `tracker::month::tests` (`month::`), and
`workspace::tracker_summarizes_and_charts_what_daily_notes_track`
(`workspace::`); `examples::every_tracker_block_in_the_example_vault_draws`
draws the example vault's (`Plugins/Tracker`).

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## Data: what is searched

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| TR-01 | `searchType` / `searchTarget`, several targets (a list or `a, b`), one type for all or one each; a dataset a target | ✅ | `params::targets_types_and_outputs_are_read` |
| TR-02 | `tag`: `#tag` counts `constValue` (1), `#tag:value` its value (units after the number are fine: `#weight:60kg`), nested tags (`#run/long`), frontmatter tags; not in code blocks | ✅ | `collect::tags_fields_properties_and_tasks_by_day` |
| TR-03 | `frontmatter` (a number, `true` / `false`, a value's part `bp[1]` by `separator`, list items), `frontmatter.exists` | ✅ | same |
| TR-04 | `dvField` (`name:: value`, `[name:: value]`, `(name:: value)`, several on a line) | ✅ | same |
| TR-05 | `task`, `task.all`, `task.done`, `task.notdone` (the target filters by text; empty: every task) | ✅ | same; `params::what_is_missing_is_said` |
| TR-06 | `wiki`: links to a note (`[[A]]`, `[[A\|alias]]`, `[[A#heading]]`) | ✅ | same |
| TR-07 | `text`: a regular expression, its `(?<value>…)` group the value, else matches counted (a plain word too) | ✅ | `collect::text_tables_and_dates_from_a_dataset` |
| TR-08 | `table`: `Note[table][column]` (a third index: a value of the cell), the dates in the `xDataset` column | ✅ | same |
| TR-09 | `fileMeta`: `cDate`, `mDate` (as dates), `size`, `numWords`, `numChars`, `numSentences` | ✅ | `collect::file_data_and_files_named` |
| TR-10 | `textValueMap`: text (an emoji) as a number, the key exact or a regular expression | ✅ | `collect::tags_fields_properties_and_tasks_by_day` |
| TR-11 | `folder`, `file`, `specifiedFilesOnly` | ✅ | `collect::file_data_and_files_named` |
| TR-12 | `fileContainsLinkedFiles` (the notes those files link to are read too), `fileMultiplierAfterLink` (a number after the link, by a regular expression's `value` group, multiplies that note's values) | ✅ | `collect::nested_keys_times_and_linked_files` |
| TR-13 | Nested frontmatter keys (`body.weight`), list items by index | ✅ | `collect::nested_keys_times_and_linked_files` |

## Data: dates and values

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| TR-20 | A note's date from its name in `dateFormat` (moment's tokens, or `iso-8601`), all of the name; `dateFormatPrefix` / `dateFormatSuffix` (regular expressions) around it | ✅ | `collect::dates_come_from_names_and_relative_dates_from_today` |
| TR-21 | `xDataset`: dates from a target (a property, a field, a table column, `cDate` / `mDate`) | ✅ | `collect::text_tables_and_dates_from_a_dataset` |
| TR-22 | `startDate` / `endDate`: a date, or relative to today (`-7d`, `-2w`, `-1M`, `-1y`, `-1y2M`, `0d`); the data's first and last days by default; a value for every day between | ✅ | `collect::ranges_penalties_shifts_and_sums_over_time` |
| TR-23 | Several values on one day (several notes, or the target more than once) are added up | ✅ | `collect::tags_fields_properties_and_tasks_by_day` |
| TR-24 | `constValue`, `ignoreAttachedValue`, `ignoreZeroValue`, `penalty`, `valueShift`, `shiftOnlyValueLargerThan`, `accum`, `datasetName`, `separator` | ✅ | `collect::ranges_penalties_shifts_and_sums_over_time` |
| TR-25 | Errors as the original's messages in place of the block ("No valid data found", "Invalid searchType", "Parameter 'searchTarget' not found or empty" …) | ✅ | `params::what_is_missing_is_said`, `workspace::` |
| TR-26 | Times (`23:15`, `22:30:30`) and durations (`7h30m`, `6h`) as values, in hours | ✅ | `collect::nested_keys_times_and_linked_files`; `valueType` isn't implemented by the original either |

## Outputs

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| TR-40 | `summary`: `template` with expressions, several lines (`\n`); `style` (CSS): `color`, `background-color`, bold, italic, underline, line-through | ✅ | `tracker::tests::margins_and_a_styled_summary`, `chart::colors_by_name_or_hex_and_styles_by_css`; sizes, fonts and spacing are a web page's |
| TR-41 | `line`: the datasets as lines (braille), points, `showLine`, `fillGap`, `lineColor`, `pointColor`; `title`, `yAxisLabel`, `yAxisUnit`, `xAxisLabel`, `yMin`, `yMax`, `reverseYAxis`, `yAxisLocation` (a right axis); the values on the axis (the top, the bottom, round values between) and the dates under it | ✅ | `chart::a_line_chart_has_axes_dates_and_braille` |
| TR-42 | `bar`: a bar a day from zero, in eighths, side by side for several datasets, `stack` (on top of each other), `barColor`; days put together when there are more days than columns | ✅ | `chart::bars_grow_from_zero_in_eighths` |
| TR-43 | Legends: `showLegend`, `legendPosition` (top, bottom, left, right), `legendOrientation`, `legendBgColor`, `legendBorderColor` (a box) | ✅ | `chart::ticks_formats_and_legends_where_asked` |
| TR-44 | `month`: weeks of the month, days passing `threshold` (`thresholdType`) circled, streaks joined (`showStreak`), today ringed, `startWeekOn`, `initMonth` (`2026-07`, or relative), `dataset`, `circleColor` / `color`, `dimNotInMonth`, `dividingLineColor`, `headerMonthColor`; a click on a day opens its note | ✅ | `month::a_month_of_weeks_with_circled_days`, `month::weeks_start_where_asked_thresholds_and_annotations`, `workspace::` |
| TR-45 | `month` `mode: annotation`, `annotation`, `showAnnotation` | ✅ | `month::weeks_start_where_asked_thresholds_and_annotations` |
| TR-46 | `month`: `‹` / `›` move between months (the title goes back), the dataset's name to the next dataset (`dataset`), a click selects a day and shows its value (`showSelectedValue`, `showSelectedRing`, `selectedRingColor`), a second click opens its note; `circleColorByValue`, `circleColorByStreak` (three strengths), `showAnnotationOfAllTargets`, `todayRingColor`, `headerYearColor` | ✅ | `month::months_datasets_and_the_day_selected`, `month::circles_by_value_or_streak`, `workspace::` |
| TR-47 | `pie`: `data` by expression, `dataName`, `dataColor`, `ratioInnerRadius` (a donut), a legend with each value and share (`legendPosition`); `label` (not under `hideLabelLessThan`) and `extLabel` (`showExtLabelOnlyIfNoLabel`) in the legend, as the sectors have no room for text | ✅ | `chart::a_pie_and_a_bullet_chart`, `chart::pie_labels_legends_and_a_standing_bullet` |
| TR-48 | `bullet`: `value` by expression, `range` (bands), `rangeColor`, `valueColor`, `valueUnit`, `showMarker`, `markerValue`, `markerColor`, `title` | ✅ | same |
| TR-49 | `bullet` `orientation: vertical`: standing, its scale on the left | ✅ | `chart::pie_labels_legends_and_a_standing_bullet` |
| TR-50 | Several outputs in one block, in the order written | ✅ | in `tracker::render` |
| TR-51 | `fixedScale`, `aspectRatio` (the plot's height); `margin` (pixels as rows and columns: 20 a row, 10 a column); charts always fit the note's width (`fitPanelWidth`) | ✅ | `tracker::tests::margins_and_a_styled_summary` |
| TR-52 | A point's value (`allowInspectData`): a click on a chart's column says its day's values in the status bar (a terminal has no hover); `xAxisTickInterval` (`1w`, `1M`, days), `xAxisTickLabelFormat`, `yAxisTickInterval`, `yAxisTickLabelFormat`; `xAxisColor`, `yAxisColor`, `xAxisLabelColor`, `yAxisLabelColor` | ✅ | `chart::a_click_on_a_column_says_its_day`, `chart::ticks_formats_and_legends_where_asked`, `workspace::` |

## Expressions

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| TR-60 | `{{expression}}` in a summary's `template`, a pie's `data` and labels, a bullet's `value` | ✅ | `expr::functions_over_the_first_dataset_or_one_given` |
| TR-61 | `dataset(i)`; `sum`, `first`, `last`, `min`, `max`, `average`, `median`, `variance`, `numTargets`, `numDays`, `numDaysHavingData` | ✅ | same |
| TR-62 | Dates: `startDate`, `endDate`, `minDate`, `maxDate`; streaks and breaks (`maxStreak`, `currentStreak`, `maxBreaks`, `currentBreaks`, each with `Start` / `End`) | ✅ | same |
| TR-63 | `normalize`, `setMissingValues`; `+ - * / %` and unary `-` on numbers and datasets (missing in either: missing) | ✅ | `expr::operators_formats_and_errors` |
| TR-64 | `::format`: printf for numbers (`%d`, `%.1f`, `%5.2f`, `i`), moment for dates (`D MMMM`) | ✅ | same |
| TR-65 | The old variables (`{{sum}}`, `{{sum(1)}}`: a dataset by number, deprecated in 1.9) | ✅ | `expr::operators_formats_and_errors` |
