import sys,time,sqlite3,json
sys.path.insert(0,'/home/ubuntu/forms-validation/money-accept-proof')
import drive_accept as d
T='00012345300000'
def reset_db():
 c=sqlite3.connect(str(d.DB));row=c.execute('select id,data_json from form_drafts where id=1').fetchone();data=json.loads(row[1]);base=data['schedule_1'][0]
 r1=dict(base);r1.update({'atc':'PT010','taxable_amount':1000.0,'tax_rate':0.03,'tax_due':30.0})
 r2=dict(base);r2.update({'atc':'PT040','taxable_amount':3000.0,'tax_rate':0.03,'tax_due':90.0})
 data['schedule_1']=[r1,r2];data['total_tax_due']=120.0;data['tax_payable']=120.0;data['total_amount_payable']=120.0
 c.execute('update form_drafts set data_json=? where id=1',(json.dumps(data,separators=(',',':')),));c.commit();c.close()
def close_form():
 if d.maybe('nav-home'):
  d.click_node('nav-home',wait=.9);time.sleep(.5)
def open_form():
 if d.maybe('home-tile-drafts'):d.click_node('home-tile-drafts',wait=.8)
 if d.maybe('return-1'):d.click_node('return-1',wait=1.3)
 if d.maybe('fp-nav-4'):d.click_node('fp-nav-4',wait=.8)
close_form();reset_db();open_form()
print('rows',[(n.get('id'),n.get('name')) for n in d.nodes() if n.get('id','').startswith('fp-row-drpATC1-')])
