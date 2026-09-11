# SPDX-License-Identifier: AGPL-3.0-or-later
import hashlib,hmac,json,os,re,socket,subprocess,tempfile,time,unittest
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
def canon(obj):
 return json.dumps(obj,sort_keys=True,separators=(",",":")).encode()
def digest_of(obj):
 return 'sha256:'+hashlib.sha256(canon(obj)).hexdigest()
def revision_signature(master,payload):
 key=hashlib.sha256(b'canopy-revision-signing-v1\x00'+master).digest()
 return 'hmac-sha256:'+hmac.new(key,canon(payload),hashlib.sha256).hexdigest()
class Daemon:
 def __init__(self,state,key):
  self.port=free_port();self.origin=f'http://127.0.0.1:{self.port}';env=os.environ.copy();env.update({'WORKFLOWD_BIND':f'127.0.0.1:{self.port}','WORKFLOWD_CONTROL_ORIGIN':self.origin,'WORKFLOWD_STATE_DIR':str(state),'WORKFLOWD_MASTER_KEY_FILE':str(key),'WORKFLOWD_ARGON_MEMORY_KIB':'8192','WORKFLOWD_ARGON_ITERATIONS':'1'});self.p=subprocess.Popen([str(BIN),'serve'],cwd=REPO,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  for _ in range(200):
   try:
    if api(self.origin,'/health/live')[0]==200:return
   except (URLError,ConnectionError):pass
   time.sleep(.03)
  raise AssertionError('not started')
 def stop(self):
  if self.p.poll() is None:self.p.terminate();self.p.wait(8)
class PublicationAcceptance(unittest.TestCase):
 def test_publication_is_deterministic_signed_and_rollback_safe(self):
  with tempfile.TemporaryDirectory() as td:
   root=Path(td);state=root/'state';key=root/'key';master=os.urandom(32);key.write_bytes(master);d=Daemon(state,key);self.addCleanup(d.stop);origin=d.origin;oh={'Origin':origin};p='/api/v1/workflows/wf-eco'
   api(origin,'/api/v1/setup','POST',{'email':'owner@example.test','password':'correct horse battery staple','recovery_passphrase':'separate recovery phrase long'},oh)
   _,rh,login=api(origin,'/api/v1/session/login','POST',{'email':'owner@example.test','password':'correct horse battery staple'},oh);auth={'Cookie':rh['set-cookie'].split(';',1)[0]};mut={**auth,'Origin':origin,'X-Canopy-CSRF':login['csrf_token']}
   html=text(origin,'/');script=re.search(r'<script type="module" src="([^"]+)"',html).group(1);bundle=text(origin,script);self.assertIn('/compile',bundle);self.assertIn('/publish',bundle);self.assertIn('/publication',bundle);self.assertIn('/rollback',bundle);self.assertIn('Draft versus Published',bundle);self.assertIn('Published Revision',bundle)
   status,_,catalog=api(origin,'/api/v1/catalog',headers=auth);self.assertEqual(status,200);lock=catalog['nodes'][0]['contract_lock']
   status,_,draft=api(origin,'/api/v1/workflows','POST',{'workflow_id':'wf-eco','name':'Eco 100K','annotation':'first publication journey','settings':{},'compatibility_metadata':{'profile':'native'}},mut);self.assertEqual(status,201);self.assertEqual(draft['draft_version'],0)
   lease=api(origin,p+'/editing/open','POST',{'editor_session_id':'ticket05-client','label':'Ticket 05 client'},mut)[2];self.assertEqual(lease['role'],'holder');generation=lease['lease_generation']
   command={'editor_session_id':'ticket05-client','lease_generation':generation,'command_id':'cmd-add-trigger','base_draft_version':0,'operation':{'kind':'add_node','node_instance':{'id':'node-trigger','name':'Start here','contract_lock':lock,'configuration':{'capture_mode':'manual'},'layout':{'x':120,'y':80},'annotation':'','compatibility_metadata':{}}}}
   status,_,accepted=api(origin,p+'/draft-commands','POST',command,mut);self.assertEqual(status,200);self.assertEqual(accepted['draft_version'],1)
   # Compile: one designated warning (unconnected trigger output) that needs acknowledgement.
   status,_,preview=api(origin,p+'/compile',headers=auth);self.assertEqual(status,200);self.assertEqual(preview['status'],'warnings');self.assertEqual(preview['draft_version'],1)
   self.assertIn('unconnected_trigger_output',[diag['code'] for diag in preview['diagnostics']]);self.assertTrue(any(diag['require_acknowledgement'] for diag in preview['diagnostics']))
   self.assertEqual(preview['plan']['plan_format'],'canopy.execution-plan/1');self.assertEqual(preview['plan']['compiler_algorithm'],'canopy.compiler/1')
   # Publish without acknowledgement is blocked with the exact required codes.
   base={'editor_session_id':'ticket05-client','lease_generation':generation,'base_draft_version':1}
   status,_,body=api(origin,p+'/publish','POST',base,mut);self.assertEqual(status,409);self.assertEqual(body['code'],'publication_warnings_unacknowledged');self.assertEqual(body['required_acknowledgements'],['unconnected_trigger_output'])
   # Publish with acknowledgement stores a signed, digested, immutable revision.
   status,_,pub1=api(origin,p+'/publish','POST',{**base,'acknowledged_warnings':['unconnected_trigger_output']},mut);self.assertEqual(status,201);self.assertEqual(pub1['revision_number'],1)
   self.assertEqual(pub1['plan_format'],'canopy.execution-plan/1');self.assertEqual(pub1['compiler_algorithm'],'canopy.compiler/1');self.assertEqual(pub1['compatibility_profile']['id'],'native')
   self.assertEqual(pub1['document_digest'],digest_of(pub1['document']))
   plan=dict(pub1['plan']);embedded=plan.pop('plan_digest');self.assertEqual(embedded,digest_of(plan));self.assertEqual(pub1['plan_digest'],embedded)
   self.assertEqual(pub1['signature'],revision_signature(master,{'algorithm':'canopy.revision-signature/1','workflow_id':'wf-eco','revision_number':1,'document_digest':pub1['document_digest'],'plan_digest':pub1['plan_digest'],'plan_format':'canopy.execution-plan/1','compiler_algorithm':'canopy.compiler/1','compatibility_profile':'native'}))
   self.assertEqual(pub1['evidence']['acknowledged_warnings'],['unconnected_trigger_output']);self.assertEqual(pub1['evidence']['signature_algorithm'],'canopy.revision-signature/1');self.assertEqual(pub1['evidence']['draft_version'],1);self.assertEqual(pub1['evidence']['editor_session_id'],'ticket05-client');self.assertEqual(pub1['evidence']['lease_generation'],generation)
   # Determinism: re-publishing the identical document yields identical digests.
   status,_,pub2=api(origin,p+'/publish','POST',base|{'acknowledged_warnings':['unconnected_trigger_output']},mut);self.assertEqual(status,201);self.assertEqual(pub2['revision_number'],2);self.assertEqual(pub2['document_digest'],pub1['document_digest']);self.assertEqual(pub2['plan_digest'],pub1['plan_digest']);self.assertNotEqual(pub2['signature'],pub1['signature'])
   # A changed document yields a changed revision with new digests.
   annotate={'editor_session_id':'ticket05-client','lease_generation':generation,'command_id':'cmd-annotate','base_draft_version':1,'operation':{'kind':'set_workflow_annotation','annotation':'published note'}}
   status,_,accepted=api(origin,p+'/draft-commands','POST',annotate,mut);self.assertEqual(status,200);self.assertEqual(accepted['draft_version'],2)
   status,_,pub3=api(origin,p+'/publish','POST',{'editor_session_id':'ticket05-client','lease_generation':generation,'base_draft_version':2,'acknowledged_warnings':['unconnected_trigger_output']},mut);self.assertEqual(status,201);self.assertEqual(pub3['revision_number'],3);self.assertNotEqual(pub3['document_digest'],pub1['document_digest'])
   # Stale base Draft Version is rejected with the current version.
   status,_,body=api(origin,p+'/publish','POST',base|{'acknowledged_warnings':['unconnected_trigger_output']},mut);self.assertEqual(status,409);self.assertEqual(body['code'],'stale_draft_version');self.assertEqual(body['current_draft_version'],2)
   # A session that does not hold the lease cannot publish.
   status,_,body=api(origin,p+'/publish','POST',{'editor_session_id':'other-client','lease_generation':0,'base_draft_version':2},mut);self.assertEqual(status,423);self.assertEqual(body['code'],'draft_lease_required')
   # A second trigger is an error: compile reports it, publish is blocked.
   second={'editor_session_id':'ticket05-client','lease_generation':generation,'command_id':'cmd-second-trigger','base_draft_version':2,'operation':{'kind':'add_node','node_instance':{'id':'node-second','name':'Second start','contract_lock':lock,'configuration':{'capture_mode':'manual'},'layout':{'x':320,'y':80},'annotation':'','compatibility_metadata':{}}}}
   status,_,accepted=api(origin,p+'/draft-commands','POST',second,mut);self.assertEqual(status,200);self.assertEqual(accepted['draft_version'],3)
   status,_,preview=api(origin,p+'/compile',headers=auth);self.assertEqual(status,200);self.assertEqual(preview['status'],'failed');self.assertIn('multiple_triggers',[diag['code'] for diag in preview['diagnostics']]);self.assertIsNone(preview.get('plan'))
   status,_,body=api(origin,p+'/publish','POST',{'editor_session_id':'ticket05-client','lease_generation':generation,'base_draft_version':3,'acknowledged_warnings':['unconnected_trigger_output']},mut);self.assertEqual(status,422);self.assertEqual(body['code'],'compilation_failed');self.assertIn('multiple_triggers',[diag['code'] for diag in body['diagnostics']])
   # The draft layer still records the command (validation belongs to the compiler); undo restores a publishable document.
   status,_,diff=api(origin,p+'/diff',headers=auth);self.assertEqual(status,200);self.assertEqual(diff['published_revision'],3);self.assertEqual([n['id'] for n in diff['nodes']['added']],['node-second'])
   for index in (3,2):
    undo={'editor_session_id':'ticket05-client','lease_generation':generation,'command_id':f'cmd-undo-{index}','base_draft_version':index,'operation':{'kind':'undo'}}
    status,_,accepted=api(origin,p+'/draft-commands','POST',undo,mut);self.assertEqual(status,200);self.assertEqual(accepted['draft_version'],index-1)
   status,_,diff=api(origin,p+'/diff',headers=auth);self.assertEqual(status,200);self.assertEqual(diff['published_revision'],3);self.assertEqual(diff['nodes']['added'],[]);self.assertEqual(diff['connections']['added'],[]);self.assertTrue(diff['workflow_fields']['annotation'])
   # Publishing the restored document reproduces the very first digests.
   status,_,pub4=api(origin,p+'/publish','POST',{'editor_session_id':'ticket05-client','lease_generation':generation,'base_draft_version':1,'acknowledged_warnings':['unconnected_trigger_output']},mut);self.assertEqual(status,201);self.assertEqual(pub4['revision_number'],4);self.assertEqual(pub4['document_digest'],pub1['document_digest']);self.assertEqual(pub4['plan_digest'],pub1['plan_digest'])
   # The visual diff against the current (r4) revision is clean: draft and published document are identical.
   status,_,diff=api(origin,p+'/diff',headers=auth);self.assertEqual(status,200);self.assertEqual(diff['published_revision'],4);self.assertFalse(diff['workflow_fields']['annotation']);self.assertEqual(diff['nodes']['added'],[]);self.assertEqual(diff['nodes']['removed'],[]);self.assertEqual(diff['nodes']['modified'],[])
   # The publication history lists every immutable revision in order.
   status,_,view=api(origin,p+'/publication',headers=auth);self.assertEqual(status,200);self.assertEqual(view['current_revision'],4);self.assertEqual([rev['revision_number'] for rev in view['revisions']],[1,2,3,4]);self.assertEqual([rev['status'] for rev in view['revisions']],['superseded','superseded','superseded','current']);self.assertEqual(view['revisions'][0]['document_digest'],pub1['document_digest'])
   record_before=api(origin,p+'/revisions/1',headers=auth)[2];self.assertEqual(record_before['status'],'superseded');self.assertEqual(record_before['document_digest'],pub1['document_digest']);self.assertEqual(record_before['plan_digest'],pub1['plan_digest']);self.assertEqual(record_before['plan'],pub1['plan']);self.assertEqual(record_before['evidence']['acknowledged_warnings'],['unconnected_trigger_output'])
   # Rollback only moves the current pointer; no revision is edited or deleted.
   for expected in (3,2,1):
    status,_,view=api(origin,p+'/rollback','POST',{},mut);self.assertEqual(status,200);self.assertEqual(view['current_revision'],expected);self.assertEqual(len(view['revisions']),4)
   status,_,body=api(origin,p+'/rollback','POST',{},mut);self.assertEqual(status,409);self.assertEqual(body['code'],'no_preceding_revision')
   status,_,record=api(origin,p+'/revisions/1',headers=auth);self.assertEqual(status,200);self.assertEqual(record['status'],'current');self.assertEqual(record['document_digest'],pub1['document_digest'])
   status,_,diff=api(origin,p+'/diff',headers=auth);self.assertEqual(status,200);self.assertEqual(diff['published_revision'],1);self.assertFalse(diff['workflow_fields']['annotation']);self.assertEqual(diff['nodes']['added'],[])
   # Restart: revisions and plans stay pinned and byte-identical (no silent recompilation).
   d.stop();d=Daemon(state,key);self.addCleanup(d.stop);origin=d.origin;oh={'Origin':origin};auth={'Cookie':api(origin,'/api/v1/session/login','POST',{'email':'owner@example.test','password':'correct horse battery staple'},oh)[1]['set-cookie'].split(';',1)[0]}
   status,_,view=api(origin,p+'/publication',headers=auth);self.assertEqual(status,200);self.assertEqual(view['current_revision'],1);self.assertEqual([rev['document_digest'] for rev in view['revisions']],[pub1['document_digest'],pub2['document_digest'],pub3['document_digest'],pub4['document_digest']])
   status,_,record=api(origin,p+'/revisions/1',headers=auth);self.assertEqual(status,200);self.assertEqual(record,record_before|{'status':'current'})
   status,_,preview=api(origin,p+'/compile',headers=auth);self.assertEqual(status,200);self.assertEqual(preview['plan']['plan_digest'],pub1['plan_digest'])
   if __name__=='__main__':unittest.main()
