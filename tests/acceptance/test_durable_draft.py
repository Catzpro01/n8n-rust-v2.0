# SPDX-License-Identifier: AGPL-3.0-or-later
import json,os,re,socket,subprocess,tempfile,time,unittest
from pathlib import Path
from urllib.request import Request,urlopen
from urllib.error import HTTPError,URLError
REPO=Path(__file__).resolve().parents[2];BIN=Path(os.environ.get('WORKFLOWD_BIN',REPO/'target/debug/workflowd'))
def free_port():
 with socket.socket() as s:s.bind(('127.0.0.1',0));return s.getsockname()[1]
def api(origin,path,method='GET',body=None,headers=None):
 req=Request(origin+path,data=None if body is None else json.dumps(body).encode(),method=method,headers={'Content-Type':'application/json',**(headers or {})})
 try:
  with urlopen(req,timeout=10) as r:return r.status,{k.lower():v for k,v in r.headers.items()},json.loads(r.read() or b'{}')
 except HTTPError as e:return e.code,{k.lower():v for k,v in e.headers.items()},json.loads(e.read() or b'{}')
def text(origin,path):
 with urlopen(origin+path,timeout=10) as r:return r.read().decode()
class Daemon:
 def __init__(self,state,key):
  self.port=free_port();self.origin=f'http://127.0.0.1:{self.port}';env=os.environ.copy();env.update({'WORKFLOWD_BIND':f'127.0.0.1:{self.port}','WORKFLOWD_CONTROL_ORIGIN':self.origin,'WORKFLOWD_STATE_DIR':str(state),'WORKFLOWD_MASTER_KEY_FILE':str(key),'WORKFLOWD_ARGON_MEMORY_KIB':'8192','WORKFLOWD_ARGON_ITERATIONS':'1'});self.p=subprocess.Popen([str(BIN),'serve'],cwd=REPO,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  for _ in range(400):
   try:
    if api(self.origin,'/health/live')[0]==200:return
   except (URLError,ConnectionError):pass
   time.sleep(.05)
  if self.p.poll() is None:self.p.terminate();self.p.wait(8)
  raise AssertionError('not started')
 def stop(self):
  if self.p.poll() is None:self.p.terminate();self.p.wait(8)
class DraftAcceptance(unittest.TestCase):
 def test_manual_trigger_draft_is_semantic_idempotent_and_durable(self):
  with tempfile.TemporaryDirectory() as td:
   root=Path(td);state=root/'state';key=root/'key';key.write_bytes(os.urandom(32));d=Daemon(state,key);self.addCleanup(d.stop);origin=d.origin;oh={'Origin':origin}
   api(origin,'/api/v1/setup','POST',{'email':'owner@example.test','password':'correct horse battery staple','recovery_passphrase':'separate recovery phrase long'},oh)
   _,rh,login=api(origin,'/api/v1/session/login','POST',{'email':'owner@example.test','password':'correct horse battery staple'},oh);auth={'Cookie':rh['set-cookie'].split(';',1)[0]};mut={**auth,'Origin':origin,'X-Canopy-CSRF':login['csrf_token']}
   html=text(origin,'/');script=re.search(r'<script type="module" src="([^"]+)"',html).group(1);bundle=text(origin,script);self.assertIn('/api/v1/session/login',bundle);self.assertIn('/draft-commands',bundle);self.assertIn('Add to new Draft',bundle)
   status,_,catalog=api(origin,'/api/v1/catalog',headers=auth);self.assertEqual(status,200);self.assertEqual(catalog['nodes'][0]['display_name'],'Manual Trigger');lock=catalog['nodes'][0]['contract_lock'];self.assertRegex(lock['digest'],r'^sha256:[0-9a-f]{64}$'); self.assertEqual(api(origin,'/catalog.v1.json')[2]['nodes'][0]['contract_lock']['digest'],lock['digest'])
   status,_,contract=api(origin,f"/api/v1/node-contracts/{lock['namespace']}/{lock['name']}/{lock['version']}",headers=auth);self.assertEqual(status,200);self.assertEqual(contract['activation']['shape'],'source');self.assertEqual(contract['effects']['class'],'pure');self.assertEqual(contract['capabilities'],[]);self.assertEqual(contract['ports']['inputs'],[]);self.assertEqual(contract['outcomes']['error_namespace'],'canopy.manual-trigger');self.assertEqual(contract['compatibility']['profile'],'native');self.assertLessEqual(contract['resources']['default']['memory_bytes'],contract['resources']['hard']['memory_bytes'])
   status,_,draft=api(origin,'/api/v1/workflows','POST',{'workflow_id':'wf-eco','name':'Eco 100K','annotation':'first durable draft','settings':{'timezone':'UTC'},'compatibility_metadata':{'profile':'native'}},mut);self.assertEqual(status,201);self.assertEqual(draft['draft_version'],0)
   lease=api(origin,'/api/v1/workflows/wf-eco/editing/open','POST',{'editor_session_id':'ticket03-client','label':'Ticket 03 client'},mut)[2];self.assertEqual(lease['role'],'holder');generation=lease['lease_generation']
   command={'editor_session_id':'ticket03-client','lease_generation':generation,'command_id':'cmd-add-trigger','base_draft_version':0,'operation':{'kind':'add_node','node_instance':{'id':'node-trigger','name':'Start here','contract_lock':lock,'configuration':{'capture_mode':'manual'},'layout':{'x':120,'y':80},'annotation':'Owner starts this Workflow','compatibility_metadata':{'external_name':'Manual Trigger'}}}}
   status,_,accepted=api(origin,'/api/v1/workflows/wf-eco/draft-commands','POST',command,mut);self.assertEqual(status,200);self.assertEqual(accepted['draft_version'],1);self.assertEqual(accepted['affected_identities'],['node-trigger']);self.assertEqual(accepted['diagnostics'],[])
   self.assertEqual(api(origin,'/api/v1/workflows/wf-eco/draft-commands','POST',command,mut)[2],accepted)
   configure={'editor_session_id':'ticket03-client','lease_generation':generation,'command_id':'cmd-configure-trigger','base_draft_version':1,'operation':{'kind':'configure_node','node_instance_id':'node-trigger','configuration':{'capture_mode':'manual'}}}; configured=api(origin,'/api/v1/workflows/wf-eco/draft-commands','POST',configure,mut)[2]; self.assertEqual(configured['draft_version'],2)
   bad_lock=json.loads(json.dumps(lock));bad_lock['digest']='sha256:'+'0'*64;bad=json.loads(json.dumps(command));bad['command_id']='cmd-bad-lock';bad['base_draft_version']=2;bad['operation']['node_instance']['id']='other';bad['operation']['node_instance']['contract_lock']=bad_lock;self.assertEqual(api(origin,'/api/v1/workflows/wf-eco/draft-commands','POST',bad,mut)[0],422)
   invalid_config={'editor_session_id':'ticket03-client','lease_generation':generation,'command_id':'cmd-bad-config','base_draft_version':2,'operation':{'kind':'configure_node','node_instance_id':'node-trigger','configuration':{'capture_mode':'manual','unknown':True}}};self.assertEqual(api(origin,'/api/v1/workflows/wf-eco/draft-commands','POST',invalid_config,mut)[0],422)
   stale={**command,'command_id':'cmd-stale','operation':{'kind':'set_workflow_annotation','annotation':'stale'}};self.assertEqual(api(origin,'/api/v1/workflows/wf-eco/draft-commands','POST',stale,mut)[0],409)
   status,_,transient=api(origin,'/api/v1/workflows/wf-eco/editor-session','POST',{'viewport':{'x':9,'y':4,'zoom':2},'selection':['node-trigger'],'open_panels':['catalog'],'search_query':'trigger'},mut);self.assertEqual(status,200);self.assertEqual(transient['draft_version'],2)
   d.stop();d=Daemon(state,key);self.addCleanup(d.stop);origin=d.origin;oh={'Origin':origin};_,rh,login=api(origin,'/api/v1/session/login','POST',{'email':'owner@example.test','password':'correct horse battery staple'},oh);auth={'Cookie':rh['set-cookie'].split(';',1)[0]};mut={**oh,**auth,'X-Canopy-CSRF':login['csrf_token']}
   status,_,loaded=api(origin,'/api/v1/workflows/wf-eco',headers=auth);self.assertEqual(status,200);self.assertEqual(loaded['draft_version'],2);self.assertEqual(loaded['annotation'],'first durable draft');self.assertEqual(loaded['nodes'][0]['layout'],{'x':120.0,'y':80.0});self.assertEqual(loaded['connections'],[]);self.assertEqual(loaded['nodes'][0]['configuration'],{'capture_mode':'manual'});self.assertEqual(loaded['settings'],{'timezone':'UTC'});self.assertEqual(loaded['compatibility_metadata'],{'profile':'native'});self.assertEqual(loaded['nodes'][0]['annotation'],'Owner starts this Workflow');self.assertEqual(loaded['nodes'][0]['compatibility_metadata'],{'external_name':'Manual Trigger'});self.assertEqual(api(origin,'/api/v1/workflows/wf-eco/draft-commands','POST',command,mut)[2],accepted);self.assertNotIn('viewport',loaded);self.assertNotIn('selection',loaded);self.assertFalse(any(k in json.dumps(loaded).lower() for k in ('sqlite','/var/lib','engine_handle','vault')))
if __name__=='__main__':unittest.main()
