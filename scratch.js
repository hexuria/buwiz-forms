const fs = require('fs');
const path = 'formtypes/2551Qv2018/formtype.json';
let data = JSON.parse(fs.readFileSync(path, 'utf8'));

let decimalFields = [
    "frm2551Qv2018:txt14",
    "frm2551Qv2018:txt15",
    "frm2551Qv2018:txt16",
    "frm2551Qv2018:txt17A",
    "frm2551Qv2018:txt17B",
    "frm2551Qv2018:txt18",
    "frm2551Qv2018:txt19",
    "frm2551Qv2018:txt20",
    "frm2551Qv2018:txt21",
    "frm2551Qv2018:txt17",
];

for (let field of data.fields) {
    if (decimalFields.includes(field.key)) {
        field.kind = 'decimal';
    }
}

fs.writeFileSync(path, JSON.stringify(data, null, 2));
console.log("Updated formtype.json to change amount fields to decimal");
