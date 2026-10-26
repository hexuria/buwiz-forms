"""Write the two demo workbooks in this folder (standard library only).

    python3 -I crates/bir-shell/demo/make_workbooks.py

Every name and amount is made up, for the test taxpayer 123-456-789-00000
(Dela Cruz Jane). Totals are formulas with their values cached, so a reader
that does not compute formulas sees the same numbers Excel shows.
"""

import os
import zipfile
from xml.sax.saxutils import escape

HERE = os.path.dirname(os.path.abspath(__file__))


def col(n):
    out = ""
    while n:
        n, r = divmod(n - 1, 26)
        out = chr(65 + r) + out
    return out


def cell(ref, value, style=0):
    s = f' s="{style}"' if style else ""
    if isinstance(value, tuple):  # (formula, cached value)
        formula, cached = value
        return f'<c r="{ref}"{s}><f>{escape(formula)}</f><v>{cached}</v></c>'
    if isinstance(value, (int, float)):
        return f'<c r="{ref}"{s}><v>{value}</v></c>'
    return f'<c r="{ref}" t="inlineStr"{s}><is><t xml:space="preserve">{escape(value)}</t></is></c>'


def sheet_xml(rows, widths):
    out = []
    for r, row in enumerate(rows, start=1):
        cells = "".join(
            cell(f"{col(c)}{r}", v[0] if isinstance(v, list) else v, v[1] if isinstance(v, list) else 0)
            for c, v in enumerate(row, start=1)
            if v is not None
        )
        out.append(f'<row r="{r}">{cells}</row>')
    cols = "".join(f'<col min="{i}" max="{i}" width="{w}" customWidth="1"/>' for i, w in enumerate(widths, start=1))
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
        f"<cols>{cols}</cols><sheetData>{''.join(out)}</sheetData></worksheet>"
    )


STYLES = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
    '<numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0.00"/></numFmts>'
    '<fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts>'
    '<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>'
    '<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>'
    '<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>'
    '<cellXfs count="4">'
    '<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>'
    '<xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/>'
    '<xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>'
    '<xf numFmtId="164" fontId="1" fillId="0" borderId="0" xfId="0" applyNumberFormat="1" applyFont="1"/>'
    "</cellXfs></styleSheet>"
)


def write_book(name, sheet_name, rows, widths):
    path = os.path.join(HERE, name)
    files = {
        "[Content_Types].xml": (
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
            '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
            '<Default Extension="xml" ContentType="application/xml"/>'
            '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
            '<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
            '<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>'
            "</Types>"
        ),
        "_rels/.rels": (
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>'
            "</Relationships>"
        ),
        "xl/workbook.xml": (
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
            'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
            f'<sheets><sheet name="{escape(sheet_name)}" sheetId="1" r:id="rId1"/></sheets></workbook>'
        ),
        "xl/_rels/workbook.xml.rels": (
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>'
            '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>'
            "</Relationships>"
        ),
        "xl/styles.xml": STYLES,
        "xl/worksheets/sheet1.xml": sheet_xml(rows, widths),
    }
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as book:
        for part, text in files.items():
            info = zipfile.ZipInfo(part, date_time=(2026, 10, 26, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            book.writestr(info, text)


def money(x):
    return round(x + 1e-9, 2)


def sales():
    months = [("Jul 2026", 182450), ("Aug 2026", 167320), ("Sep 2026", 195880)]
    head = 5
    rows = [
        [["Dela Cruz Jane - Quarterly sales, Q3 2026 (BIR Form 2551Q)", 1]],
        ["TIN 123-456-789-00000 - SAMPLE DATA: every name and amount is made up."],
        ["Non-VAT. Percentage tax under Sec. 116 (ATC PT010) at 3% of gross sales."],
        [],
        [["Month", 1], ["Gross sales", 1], ["ATC", 1], ["Tax rate", 1], ["Percentage tax", 1]],
    ]
    for i, (month, amount) in enumerate(months):
        r = head + 1 + i
        rows.append([month, [amount, 2], "PT010", 0.03, [(f"ROUND(B{r}*D{r},2)", money(amount * 0.03)), 2]])
    first, last = head + 1, head + len(months)
    total = sum(a for _, a in months)
    rows.append([
        ["Total", 1],
        [(f"SUM(B{first}:B{last})", total), 3],
        ["PT010", 1],
        0.03,
        [(f"SUM(E{first}:E{last})", money(sum(money(a * 0.03) for _, a in months))), 3],
    ])
    write_book("2551Q-2026-Q3-sales.xlsx", "Q3 2026 sales", rows, [14, 16, 9, 10, 17])


def payroll():
    # name, position, basic pay, overtime pay, de minimis, SSS, PhilHealth, Pag-IBIG, tax withheld
    staff = [
        ("Santos, Maria", "Store supervisor", 35000, 0, 2000, 1000, 875, 200, 1813.80),
        ("Reyes, Jose", "Cashier", 22000, 0, 1500, 1100, 550, 200, 0.00),
        ("Cruz, Ana", "Bookkeeper", 28000, 0, 1500, 1250, 700, 200, 752.55),
        ("Bautista, Pedro", "Delivery driver", 21000, 1800, 1000, 1050, 525, 200, 28.80),
        ("Mendoza, Liza", "Sales associate", 20000, 0, 1000, 1000, 500, 200, 0.00),
    ]
    head = 5
    rows = [
        [["Dela Cruz Jane - Payroll, September 2026 (BIR Form 1601-C)", 1]],
        ["TIN 123-456-789-00000 - SAMPLE DATA: every name and amount is made up."],
        ["No minimum wage earners. No 13th month pay this month. Tax withheld per the BIR withholding tax table (monthly)."],
        [],
        [[h, 1] for h in [
            "Employee", "Position", "Basic pay", "Overtime pay", "De minimis benefits",
            "Gross compensation", "SSS", "PhilHealth", "Pag-IBIG", "Statutory contributions",
            "Taxable compensation", "Tax withheld",
        ]],
    ]
    for i, (name, position, basic, ot, dm, sss, ph, hdmf, tax) in enumerate(staff):
        r = head + 1 + i
        gross = basic + ot + dm
        stat = sss + ph + hdmf
        taxable = gross - dm - stat
        # The table: 0 up to 20,833; 15% of the excess over 20,833 up to 33,332.
        expected = money(max(0, taxable - 20833) * 0.15)
        assert expected == tax, (name, expected, tax)
        rows.append([
            name, position, [basic, 2], [ot, 2], [dm, 2],
            [(f"C{r}+D{r}+E{r}", gross), 2], [sss, 2], [ph, 2], [hdmf, 2],
            [(f"G{r}+H{r}+I{r}", stat), 2], [(f"F{r}-E{r}-J{r}", taxable), 2], [tax, 2],
        ])
    first, last = head + 1, head + len(staff)
    sums = {}
    for c, letter in enumerate("CDEFGHIJKL", start=3):
        idx = c - 3
        values = []
        for (name, position, basic, ot, dm, sss, ph, hdmf, tax) in staff:
            gross = basic + ot + dm
            stat = sss + ph + hdmf
            values.append([basic, ot, dm, gross, sss, ph, hdmf, stat, gross - dm - stat, tax][idx])
        sums[letter] = money(sum(values))
    rows.append([["Total", 1], None] + [[(f"SUM({l}{first}:{l}{last})", sums[l]), 3] for l in "CDEFGHIJKL"])
    write_book("1601C-2026-09-payroll.xlsx", "Sep 2026 payroll", rows, [18, 17, 12, 13, 12, 14, 9, 11, 10, 14, 14, 12])


if __name__ == "__main__":
    sales()
    payroll()
