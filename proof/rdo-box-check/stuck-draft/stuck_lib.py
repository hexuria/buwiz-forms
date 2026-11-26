import sys,json,sqlite3,os,time,subprocess
sys.path.insert(0,'/home/ubuntu/forms-validation/rdo-stuck-proof')
import drive_stuck as d
OUT=d.OUT
TIN='00011124500000'

def connect(db):
    return sqlite3.connect(str(OUT/db),timeout=3)

def profile_rdo(db):
    con=connect(db); row=con.execute('select data_json from profiles where tin=?',(TIN,)).fetchone(); con.close()
    return json.loads(row[0]).get('rdo_code') if row else None

def set_profile_rdo(db,value):
    con=connect(db)
    row=con.execute('select data_json from profiles where tin=?',(TIN,)).fetchone()
    data=json.loads(row[0]);data['rdo_code']=value
    con.execute('update profiles set data_json=? where tin=?',(json.dumps(data),TIN))
    con.commit();con.close()

def drafts(db):
    con=connect(db)
    rows=[]
    for r in con.execute("select id,tin,form_code,taxable_year,quarter,status,data_json,period_key from form_drafts where tin=? and form_code='1601C' order by quarter",(TIN,)):
        data=json.loads(r[6]); rows.append({'id':r[0],'tin':r[1],'form_code':r[2],'year':r[3],'quarter':r[4],'status':r[5],'period_key':r[7],'data':data})
    con.close();return rows

def summarize_draft(row):
    x=row['data']
    return {'id':row['id'],'quarter':row['quarter'],'period_key':row['period_key'],'status':row['status'],'rdo':x.get('rdo_code'),'month':x.get('month'),'profile_edits':x.get('profile_edits'),'profile_baseline':x.get('profile_baseline'),'updated_at':x.get('updated_at')}

def clone_row(db,srcquarter,dstquarter,rdo=None,legacy=False,edited=False,baseline_mode='source'):
    con=connect(db)
    r=con.execute("select data_json,created_at from form_drafts where tin=? and form_code='1601C' and taxable_year=2026 and quarter=?",(TIN,srcquarter)).fetchone()
    if not r: raise RuntimeError(f'no source draft month {srcquarter}')
    data=json.loads(r[0])
    data['id']=None;data['month']=dstquarter;data['taxable_year']=2026
    if rdo is not None:data['rdo_code']=rdo
    if legacy:
        data.pop('profile_baseline',None);data.pop('profile_edits',None)
    else:
        if edited:
            data['profile_edits']=['frm1601c:txtRDOCode']
        else:
            data['profile_edits']=[]
        if baseline_mode=='empty':data['profile_baseline']={'/rdo_code':''}
        elif baseline_mode=='050':data['profile_baseline']={'/rdo_code':'050'}
    now='2026-11-26T11:00:00+00:00';data['created_at']=now;data['updated_at']=now
    key=f'M{dstquarter}'
    con.execute("delete from form_drafts where tin=? and form_code='1601C' and taxable_year=2026 and quarter=?",(TIN,dstquarter))
    con.execute("insert into form_drafts(tin,form_code,taxable_year,quarter,status,data_json,period_key,created_at,updated_at) values(?,?,?,?,?,?,?,?,?)",(TIN,'1601C',2026,dstquarter,'Draft',json.dumps(data),key,now,now))
    con.commit();con.close()

def delete_drafts(db):
    con=connect(db);con.execute("delete from form_drafts where form_code='1601C'");con.commit();con.close()

def copy_1601_rows(srcdb,dstdb):
    con=connect(srcdb);rows=con.execute("select tin,form_code,taxable_year,quarter,status,data_json,period_key,created_at,updated_at from form_drafts where tin=? and form_code='1601C'",(TIN,)).fetchall();con.close()
    con=connect(dstdb);con.execute("delete from form_drafts where tin=? and form_code='1601C'",(TIN,))
    for row in rows:con.execute("insert into form_drafts(tin,form_code,taxable_year,quarter,status,data_json,period_key,created_at,updated_at) values(?,?,?,?,?,?,?,?,?)",row)
    con.commit();con.close()

def rdo_state(db,quarter):
    rows=[x for x in drafts(db) if x['quarter']==quarter]
    return summarize_draft(rows[0]) if rows else None
