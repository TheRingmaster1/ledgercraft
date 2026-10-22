# Sales tax

How LedgerCraft charges, records and reports US sales tax, and where its
rates come from. The bookkeeping (postings, rounding, what is stored on an
invoice) is in [accounting.md](accounting.md); the file fields are in
[file-format.md](file-format.md).

## Which states are supported

**Importing rates is supported for the states in the Streamlined Sales Tax
(SST) agreement, and only those, until a source as reliable and free as
theirs is published for the other states.**

The SST Governing Board publishes, free, a *rate file* and a *boundary file*
for each member state, updated quarterly, at
<https://www.streamlinedsalestax.org/Shared-Pages/rate-and-boundary-files>
(the files themselves are listed at
<https://www.streamlinedsalestax.org/ratesandboundry/Rates/> and
`…/ratesandboundry/Boundary/`). These are the states with files there:

| | | | |
|---|---|---|---|
| Arkansas (AR) | Iowa (IA) | Michigan (MI) | Nevada (NV) |
| Georgia (GA) | Kansas (KS) | Minnesota (MN) | New Jersey (NJ) |
| Indiana (IN) | Kentucky (KY) | Nebraska (NE) | North Carolina (NC) |
| North Dakota (ND) | Oklahoma (OK) | South Dakota (SD) | Utah (UT) |
| Ohio (OH) | Rhode Island (RI) | Tennessee (TN) | Vermont (VT) |
| Washington (WA) | West Virginia (WV) | Wisconsin (WI) | Wyoming (WY) |

* **Checked:** Utah. For all 350 Utah locations in the October 2026 files,
  the imported rates add up to the combined rate in Utah's own published
  list. The other states use the same file format and should import the same
  way, but have not been checked against their state's list yet.
* **Vermont** may not import: its boundary file is reported to locate taxes
  by street address rather than ZIP+4, which LedgerCraft cannot read yet. The
  import says so rather than guessing.
* **Every other state** (California, Texas, New York, Florida, Illinois and
  the rest): add your taxes and groups by hand in Lists > Sales Tax, using the
  rates your state publishes. Support will be added when a dependable, free,
  machine-readable source for them is listed alongside the SST files. Paid
  rate services and scraping state websites are deliberately not used.
* States with no sales tax (Alaska at state level, Delaware, Montana, New
  Hampshire, Oregon) need nothing.

## Getting the files

1. Open the Rate and Boundary Files page above, choose **Rate Files**, and
   download your state's file; then **Boundary Files**, and its boundary file.
   They are named `<state><R or B><year>Q<quarter><month><day>`, e.g.
   `UTR2026Q4AUG11.csv` and `UTB2026Q4AUG11.csv`.
2. Some states publish `.csv`, others `.zip`: unzip a `.zip` first.
3. A date years old (for example `INR2008Q4MAY7`) is not a problem: a state
   posts a new file only when something changes.
4. In LedgerCraft: Lists > Sales Tax > Import Rates from State Files…, choose
   both files and enter your ZIP+4. LedgerCraft remembers the files for that
   state on this computer, for Find from address on customers.

## Charging it

* A *sales tax* is one agency's rate with the date each rate starts; a *tax
  group* is the taxes charged together at one location. Invoices charge a
  group; a customer can have a default group or be exempt.
* The rates in force on the invoice's date are used, and what was charged is
  stored on the invoice, so later rate changes never alter it.
* Whether a sale is taxed at your location or the customer's ("sourcing")
  depends on each state's rules. LedgerCraft lets you choose either per
  customer or invoice; it does not decide for you.

## Keeping rates current

States in the agreement change rates only at the start of a calendar quarter
and publish them by the first day of the month before. From March, June,
September and December 1, LedgerCraft reminds you to check, until you import
the new files or mark the rates as checked.

## Responsibility

LedgerCraft reads the files states publish; it does not give tax advice, and
the business remains responsible for charging and paying the right tax.
