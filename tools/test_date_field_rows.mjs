import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root=new URL("../",import.meta.url);
const schema=JSON.parse(fs.readFileSync(new URL("app/forms/screen-1700-v2018.json",root)));
const cat=schema.parts.flatMap((p,pi)=>p.items.map((it,ii)=>({it,pi,ii}))).find(x=>(x.it.f||[]).some(f=>/BirthDate$/.test(f.k)));
assert.ok(cat,"real 1700 date of birth item");
const dateKey=cat.it.f[0].k;
const traces=[], entities=[], tasks=[];
let rootDirty=true, owner=null, previewCalls=0, column=1056, errors=[], holdValidation=false, finishValidation;
const checkButtons=[], chips=[];
class View {}
function context(entity=null){
 return {notify(){if(entity)entity.dirty=true;else rootDirty=true;},
 new(Class,props){const e={dirty:true,released:false,set_props(p){e.props=p;e.view.update?.(p);e.dirty=true;},release(){e.released=true;return true;}};
 e.view=new Class(props);e.props=props;entities.push(e);const old=owner;owner=e;e.view.init?.(props,context(e));owner=old;return e;},
 spawn(fn){const p=Promise.resolve(fn(context(entity)));tasks.push(p);return {cancel(){}};},
 timer:{after(){return {cancel(){}};},every(){return {cancel(){}};}},
 theme(){return {appearance:"light",is_dark:false,colors:{}};}
 };
}
const clicks=new Map();
function element(){
 const creationOwner=owner;
 const target={};
 let el=new Proxy(target,{get(o,k){
 if(k==="then")return undefined;
 return (...args)=>{
 if(k==="id"){o.id=args[0];if(String(o.id).startsWith("fp-item-"))traces.push(o.id);}
 if(k==="on_click")clicks.set(o.id,{fn:args[0],entity:creationOwner});
 if(k==="when"&&args[0])args[1](el);
 if(k==="when_some"&&args[0]!=null)args[1](el,args[0]);
 if(k==="child")mount(args[0]);
 if(k==="children")for(const c of args[0])mount(c);
 return el;
 };}});
 return el;
}
function mount(e){if(e?.view && e.dirty&&!e.released){e.dirty=false;const old=owner;owner=e;e.view.render(context(e));owner=old;}}
const inputs=[];
const InputState={new({value=""}={}){let text=value;const h={};const i={value:()=>text,set_value:v=>{text=v;},on(n,f){h[n]=f;},fire(n,c){h[n]?.({},c);},type(v,c){text=v;h.change?.({},c);},release(){return true;}};inputs.push(i);return i;}};
const CalendarState={new(){let v=null;return {set_value(x){v=x;},value:()=>v,on(){},month:()=>12,year:()=>2020,month_days:()=>[[]],prev_month(){},next_month(){}};}};
const contextVM=vm.createContext({console,Map,Set,JSON,window:{viewport_size:()=>({width:1400,height:900})}});
function mock(name,exports){const m=new vm.SyntheticModule(Object.keys(exports),function(){for(const [k,v]of Object.entries(exports))this.setExport(k,v);},{context:contextVM,identifier:name});return m;}
const kit=mock("gpui-kit",{div:element,svg:element,View});
const base=mock("gpui-base",{InputState,CalendarState,Popup:{new:element}});
const components=mock("gpui-component",{Spinner:{new:element}});
const uiExports={tokens:()=>new Proxy({},{get:()=>"#000"}),SPACE:[0,4,8,12,16,20,24],RADIUS:{},SIZE:{validationLine:18},BREAK:{formNav:760}};
for(const n of ["Text","Line","Code","Icon","AppButton","IconButton","Avatar","Chip","Switch","Checkbox","TextField","Dropdown","PageHeader","MetaLine"])uiExports[n]=element;
uiExports.AppButton=(_cx,id,label,opts={})=>{
 if(id==="fp-check")checkButtons.push({label,ariaLabel:opts.ariaLabel,leading:opts.leading});
 return element();
};
uiExports.IconButton=(_cx,id,_icon,opts={})=>{const owner_=owner;clicks.set(id,{fn:opts.onClick,entity:owner_});return element();};
uiExports.Chip=(_cx,label)=>{chips.push(label);return element();};
const uiModule=mock("./ui.js",uiExports);
const store=mock("./store.js",{formatTin:x=>x,errorText:e=>String(e&&e.message?e.message:e)});
const periods=mock("./periods.js",{periodLabel:()=>"March 2026"});
const mod=new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js",root),"utf8"),{context:contextVM});
await mod.link(s=>({"gpui-kit":kit,"gpui-base":base,"gpui-component":components,"./ui.js":uiModule,"./store.js":store,"./periods.js":periods}[s]));
await mod.evaluate();
let values={[dateKey]:""};
const app={prefs:{},branchFor:()=>null,taxpayers:()=>[],layout:()=>({column}),invalidate:()=>{rootDirty=true;},toast(){rootDirty=true;},store:{templates:()=>[],call(name,...args){
 if(name==="screen_form_schema")return schema;
 if(name==="screen_form_open")return Promise.resolve({draft:{id:1},status:"Draft",editable:true,values:{...values},locks:{"frm1700:txtPg1I7TaxpayerName":"comes from the taxpayer profile"},profile:["frm1700:txtPg1I7TaxpayerName"],errors:[]});
 if(name==="screen_form_fill"){values={...values,...args[2]};return{draft:{id:1},values:{...values},options:{},rejected:{}};}
 if(name==="screen_form_validate")return Promise.resolve({draft:args[1],values:{...values},options:{},errors});
 if(name==="screen_form_preview_sync"){previewCalls++;return true;}
 throw Error(name);
 }}};
const P=mod.namespace;
for(const t of ["12012020","12/01/2020","12-01-2020","2020-12-01","Dec 01 2020","12/1/2020"])assert.equal(P.formatMDY(P.parseTypedDate(t)),"12/01/2020",t);
for(const t of ["13012020","02302020","1201202","abc","00/00/0000"])assert.equal(P.parseTypedDate(t),null,t);
app.formUi=new P.FormPageActions(app);
const cx=context();
app.formUi.open({code:"1700",tin:"12345678800000",year:2026,period:0},cx);
async function settle(){while(tasks.length)await Promise.all(tasks.splice(0));}
await settle();
app.formUi.jumpTo(cat.pi,cx);
app.formUi.state.previewOpen=true;
function frame(){if(rootDirty){rootDirty=false;owner=null;P.renderFormPage(cx,app,column);}for(const e of entities)mount(e);}
frame();
const row="fp-item-"+cat.pi+":"+cat.ii;
const input=app.formUi.state.inputs.get(dateKey);
assert.ok(input,"date box has an input");
assert.ok(clicks.get("fp-date-"+cat.pi+":"+cat.ii),"date box has a calendar button");
// Typing eight digits, one keystroke at a time.
traces.length=0;let root0=rootDirty;
for(const ch of "12012020".split("").map((_,i,a)=>a.slice(0,i+1).join(""))){input.type(ch,cx);frame();}
const typing={rowBuilds:traces.length,distinctRows:[...new Set(traces)],rootDirty};
assert.equal(input.value(),"12012020","typing is left as typed");
assert.equal(app.formUi.state.pending[dateKey],"12/01/2020","draft gets MM/DD/YYYY");
input.fire("blur",cx);frame();await settle();frame();
assert.equal(input.value(),"12/01/2020","blur normalizes the box");
assert.equal(values[dateKey],"12/01/2020");
const afterBlur=[...new Set(traces)];
assert.deepEqual(afterBlur.filter(r=>r!==row),[],"only the date row rebuilds while typing and on blur");
// Invalid date: message on that row only.
traces.length=0;input.type("13312020",cx);input.fire("blur",cx);frame();await settle();frame();
assert.match(app.formUi.state.dateErrors[cat.pi+":"+cat.ii],/valid date/);
assert.deepEqual([...new Set(traces)],[row],"invalid date marks only its row");
input.type("12012020",cx);input.fire("blur",cx);frame();await settle();frame();
assert.equal(app.formUi.state.dateErrors[cat.pi+":"+cat.ii],undefined,"fixed date clears the message");
// Open and close the calendar.
traces.length=0;
const btn=clicks.get("fp-date-"+cat.pi+":"+cat.ii);
const press=()=>{const old=owner;owner=btn.entity;btn.fn({},context(btn.entity));owner=old;frame();};
press();assert.ok(app.formUi.state.picker,"calendar opens");
const openBuilds=traces.length;
const btn2=clicks.get("fp-date-"+cat.pi+":"+cat.ii);
traces.length=0;{const old=owner;owner=btn2.entity;btn2.fn({},context(btn2.entity));owner=old;frame();}
assert.equal(app.formUi.state.picker,null,"calendar closes");
const closeBuilds=traces.length;
// Pick a date through the calendar.
press();traces.length=0;
app.formUi.pickDate({single:cat.it.f[0]},"2019-03-07",cx);frame();await settle();frame();
assert.equal(input.value(),"03/07/2019");assert.equal(values[dateKey],"03/07/2019");
const pickRows=[...new Set(traces)];
assert.deepEqual(pickRows,[row],"picking a date rebuilds only its row");
console.log(JSON.stringify({typing8Keys:typing,picker:{openBuilds,closeBuilds,pickRows:pickRows.length}}));
console.log("PASS: typed dates normalize to MM/DD/YYYY, calendar fills the box, only the date row rebuilds.");
