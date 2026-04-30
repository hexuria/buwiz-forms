const fs = require('fs');
const path = require('path');

function fixFormType(formDir) {
    const jsonPath = path.join('formtypes', formDir, 'formtype.json');
    if (!fs.existsSync(jsonPath)) return;
    
    let data = JSON.parse(fs.readFileSync(jsonPath, 'utf8'));
    
    // Identifiers for money fields
    const moneyPatterns = [/txt\d{2}$/, /Amount/i, /TaxDue/i, /TaxWithheld/i, /Surcharge/i, /Interest/i, /Compromise/i, /Penalties/i, /Amt/i, /Due/i];

    data.fields.forEach(f => {
        let isMoney = moneyPatterns.some(p => p.test(f.key));
        if (formDir === '2551Qv2018') {
             if (f.key === 'frm2551Qv2018:txt17') isMoney = true;
             if (f.key.toLowerCase().includes('rate')) isMoney = false;
        }

        if (isMoney) {
            f.kind = 'dec';
            f.direction = 'Ltr'; // bir-print handles RTL inside the render macro
        } else if (f.key.toLowerCase().includes('rate')) {
            f.kind = 'char';
            f.direction = 'Rtl';
        }
    });

    fs.writeFileSync(jsonPath, JSON.stringify(data, null, 2));
    console.log(`Updated ${jsonPath}`);
}

['2551Qv2018', '1701Qv2018'].forEach(fixFormType);
