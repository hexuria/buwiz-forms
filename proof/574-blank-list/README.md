# #574 step 1: blank-box probe on main 5f978bd5628c2f9962908f8feced5fc04f629b50

`probe-results.txt` — each line `PROBE|slug|writer key|verdict|note`: the key
was filled with a sentinel value (1357 / ZXQW…) and the printed sheet read
through the real print path `staticize(prepare_html_for(slug, &filled_document(..), true))`.
PRINTS = the value appears; BLANK = the print leaves the box blank.

Still blank on main: 1801 date-of-death YEAR (month/day print), contact number,
total payable; 1701MS 25A/27A/29A/30A; 1707A Schedule 1 total selling price/cost,
Schedule 2 total selling price/cost, line of business (the capital gains/loss
and tax-paid totals DO print); 2200M region/province/city codes items 10+11;
2200P line of business. Now printing (fixed since the old list): 1801 installment
year, 1707A S1 capital gains + tax paid + S2 capital loss, 1707 seller/buyer address.
Not probed: 1707 page-2 address block variant, 1701A line of business (no key in probe).
