# R-01 decision: Should the collapsed right panel rail run the full height beside the Now band and the phase strip?
Finding: UXF-004. Target metric: contract.scroll@1366x768, now 1 in every scenario (R-01), expected 0.
Yes: the rail is fixed at the right edge from under the KPI strip to the bottom; the Now band and the phase strip give it 44 px (40 px up to 1400 px wide). Built in change C4. Shot: shots/decision-yes.png
No:  the rail stays inside the bottom grid, where its seven vertical tab names (about 490 px) are taller than the grid row at 1366x768, so the document scrolls. Shot: shots/decision-no.png
Default if unanswered by the next round: yes, because it is the only change in R-01 that turns hard gate 1 green without dropping a tab name, and it is undone by removing one CSS block.
Answer:
