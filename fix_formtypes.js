const fs = require('fs');
const path = require('path');

function fixFormType(formDir) {
    const jsonPath = path.join('formtypes', formDir, 'formtype.json');
    if (!fs.existsSync(jsonPath)) return;
    
    let data = JSON.parse(fs.readFileSync(jsonPath, 'utf8'));
    
    // Identifiers for money fields
    const moneyPatterns = [/txt\d{2}$/, /Amount/i, /TaxDue/i, /TaxWithheld/i, /Surcharge/i, /Interest/i, /Compromise/i, /Penalties/i];
    const ratePatterns = [/Rate/i];

    data.fields.forEach(f => {
        let isMoney = moneyPatterns.some(p => p.test(f.key));
        // txt17 in 2551Q is tax rate (not amount) - well, wait.
        if (formDir === '2551Qv2018') {
             if (f.key === 'frm2551Qv2018:txt17') isMoney = true; // Wait, txt17 is Tax Due on item 17!
             if (f.key === 'frm2551Qv2018:txtRate') isMoney = false;
        }

        if (isMoney) {
            f.kind = 'dec';
        } else if (f.key.toLowerCase().includes('rate')) {
            f.kind = 'char';
            f.direction = 'Rtl';
        } else if (f.kind === 'decimal') { // legacy fix
            f.kind = 'dec';
        } else if (f.kind === 'integer') {
            f.kind = 'int';
        } else if (f.kind === 'checkbox') {
            f.kind = 'bool';
        } else if (f.kind === 'string' || f.kind === 'text') {
            f.kind = 'char';
        }
        
        // Ensure implicitly we don't have integer for money
    });

    fs.writeFileSync(jsonPath, JSON.stringify(data, null, 2));
    console.log(`Updated ${jsonPath}`);
}

['2551Qv2018', '1701Qv2018'].forEach(fixFormType);
