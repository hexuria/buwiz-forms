# What the official program prints past the paper's rows

Research, report only. Source: hexuria/buwiz-validation clone (a byte-exact copy of the BIR
Offline eBIRForms package as published at bir.gov.ph/ebirforms; nothing patched, per its README),
at commit ea8334a. The package/ tree carries "Release Notes 7.9.6.1.txt" and the repo has per-release
manifests 7.9.6.0/7.9.6.1/7.9.6.2/7.9.6.2.1; the commit log shows 7.9.6.2.1 then 7.9.6.2 added on top.

HASH VERIFICATION against the buwiz-forms pins (rules/forms/*/manifest.json official-hta-runtime):
35 of 38 pinned form HTAs match the pin byte for byte (sha256). Three mismatch:
1601fq-v2018, 1603q-v2018, 1702q-v2018c — these three forms changed in the 7.9.6.2 line after the
buwiz-forms pins were captured; none of them is a growable-table form in the report below.
The BIRForms.exe executable itself is not in the buwiz-validation package/ tree (it ships forms,
not the launcher), so the package-exe pin de8ef0815… cannot be re-checked here; the per-form HTA
pins are the stronger check and they hold for every form this report uses.
