# SPDX-License-Identifier: AGPL-3.0-or-later
from __future__ import annotations
import hashlib, json, os, socket, subprocess, tempfile, time, unittest
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

REPO=Path(__file__).resolve().parents[2]
BIN=Path(os.environ.get('WORKFLOWD_BIN', REPO/'target/debug/workflowd'))

def port():
    with socket.socket() as s: s.bind(('127.0.0.1',0)); return s.getsockname()[1]

def call(origin,path,method='GET',body=None,headers=None):
    data=None if body is None else json.dumps(body).encode()
    req=Request(origin+path,data=data,method=method,headers={'Content-Type':'application/json',**(headers or {})})
    def decode(raw):
        try: return json.loads(raw or b'{}')
        except json.JSONDecodeError: return {'raw':raw.decode(errors='replace')}
    try:
        with urlopen(req,timeout=10) as r: return r.status,{k.lower():v for k,v in r.headers.items()},decode(r.read())
    except HTTPError as e: return e.code,{k.lower():v for k,v in e.headers.items()},decode(e.read())

class Daemon:
    def __init__(self,state,key,**extra):
        self.port=port(); self.origin=f'http://127.0.0.1:{self.port}'
        env=os.environ.copy(); env.update({'WORKFLOWD_BIND':f'127.0.0.1:{self.port}','WORKFLOWD_CONTROL_ORIGIN':self.origin,'WORKFLOWD_STATE_DIR':str(state),'WORKFLOWD_MASTER_KEY_FILE':str(key),'WORKFLOWD_ARGON_MEMORY_KIB':'8192','WORKFLOWD_ARGON_ITERATIONS':'1',**extra})
        self.p=subprocess.Popen([str(BIN),'serve'],cwd=REPO,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
        for _ in range(400):
            if self.p.poll() is not None: raise AssertionError(self.p.stdout.read())
            try:
                if call(self.origin,'/health/live')[0]==200: break
            except (URLError,ConnectionError): pass
            time.sleep(.05)
        else:
            self.p.terminate(); self.p.wait(8)
            raise AssertionError('daemon did not start')
    def stop(self):
        if self.p.poll() is None: self.p.terminate(); self.p.wait(5)
        if self.p.stdout.closed: return ''
        output=self.p.stdout.read(); self.p.stdout.close(); return output

class OwnerRecoveryAcceptance(unittest.TestCase):
    def test_owner_session_recovery_and_route_authority(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td); state=root/'state'; key=root/'master.key'; key.write_bytes(os.urandom(32)); key.chmod(0o600)
            d=Daemon(state,key); self.addCleanup(d.stop); origin=d.origin; h={'Origin':origin}
            status,_,health=call(origin,'/health/ready')
            self.assertEqual(status,200); self.assertEqual(health['recovery']['state'],'setup-required')
            request={'email':'owner@example.test','password':'correct horse battery staple','recovery_passphrase':'separate recovery phrase with enough length'}
            status,_,setup=call(origin,'/api/v1/setup','POST',request,h)
            self.assertEqual(status,201); self.assertEqual(setup['owner']['email'],'owner@example.test')
            self.assertEqual(setup['recovery_kit']['checksum'],hashlib.sha256(setup['recovery_kit']['document'].encode()).hexdigest())
            self.assertNotIn(request['password'],json.dumps(setup)); self.assertNotIn(request['recovery_passphrase'],json.dumps(setup))
            self.assertEqual(call(origin,'/api/v1/setup','POST',request,h)[0],409)
            status,headers,login=call(origin,'/api/v1/session/login','POST',{'email':request['email'],'password':request['password']},{**h,'Cookie':'canopy_session=attacker-fixed'})
            self.assertEqual(status,200); cookie=headers['set-cookie']; self.assertIn('HttpOnly',cookie); self.assertIn('Secure',cookie); self.assertIn('SameSite=Strict',cookie); self.assertNotIn('attacker-fixed',cookie)
            auth={'Origin':origin,'Cookie':cookie.split(';',1)[0],'X-Canopy-CSRF':login['csrf_token']}
            self.assertGreaterEqual(setup['owner']['password_kdf']['measured_millis'],0)
            wrong_csrf={**auth,'X-Canopy-CSRF':'wrong-proof'}; self.assertEqual(call(origin,'/api/v1/recovery/acknowledge','POST',{'checksum':setup['recovery_kit']['checksum']},wrong_csrf)[0],403)
            self.assertEqual(call(origin,'/api/v1/recovery/acknowledge','POST',{'checksum':setup['recovery_kit']['checksum']},auth)[0],204)
            self.assertEqual(call(origin,'/health/ready')[2]['recovery']['state'],'local-recovery-only')
            self.assertEqual(call(origin,'/api/v1/session/renew','POST',{}, {'Cookie':auth['Cookie'],'X-Canopy-CSRF':auth['X-Canopy-CSRF']})[0],403)
            self.assertEqual(call(origin,'/public/v1/setup','POST',request,h)[0],404)
            for name in ('backup','restore','update','drafts','credentials'):
                self.assertEqual(call(origin,f'/public/v1/{name}')[0],404)
            status,_,audit=call(origin,'/api/v1/audit',headers={'Cookie':auth['Cookie']})
            self.assertEqual(status,200); self.assertTrue(all(e['actor_id']=='owner:1' for e in audit['events']))
            serialized=json.dumps(audit); self.assertNotIn(request['password'],serialized); self.assertNotIn(login['csrf_token'],serialized)
            logs=d.stop(); self.assertNotIn(request['password'],logs); self.assertNotIn(request['recovery_passphrase'],logs); self.assertNotIn(login['csrf_token'],logs)

    def test_key_hash_session_limits_and_safe_failures(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td); state=root/'state'; key=root/'master.key'; key_bytes=os.urandom(32); key.write_bytes(key_bytes); key.chmod(0o600)
            password='correct horse battery staple'; recovery='separate recovery phrase with enough length'; email='owner@example.test'
            first=Daemon(state,key); self.addCleanup(first.stop); h={'Origin':first.origin}
            setup=call(first.origin,'/api/v1/setup','POST',{'email':email,'password':password,'recovery_passphrase':recovery},h)[2]
            logs=first.stop()
            database=(state/'workflow.sqlite3').read_bytes()
            self.assertNotIn(key_bytes,database); self.assertNotIn(password.encode(),database); self.assertNotIn(recovery.encode(),database)
            self.assertNotIn(setup['recovery_kit']['document'].encode(),database)

            def failed(extra):
                p=port(); env=os.environ.copy(); env.update({'WORKFLOWD_BIND':f'127.0.0.1:{p}','WORKFLOWD_CONTROL_ORIGIN':f'http://127.0.0.1:{p}','WORKFLOWD_STATE_DIR':str(state),'WORKFLOWD_ARGON_MEMORY_KIB':'8192','WORKFLOWD_ARGON_ITERATIONS':'2',**extra})
                return subprocess.run([str(BIN),'serve'],cwd=REPO,env=env,capture_output=True,text=True,timeout=5)
            missing=failed({}); self.assertNotEqual(missing.returncode,0); self.assertIn('master key is required',missing.stderr)
            wrong=root/'wrong.key'; wrong.write_bytes(os.urandom(32)); bad=failed({'WORKFLOWD_MASTER_KEY_FILE':str(wrong)}); self.assertNotEqual(bad.returncode,0); self.assertIn('vault key rejected',bad.stderr)

            daemon=Daemon(state,key,WORKFLOWD_ARGON_ITERATIONS='2'); self.addCleanup(daemon.stop); origin=daemon.origin; headers={'Origin':origin}
            status,response_headers,grant=call(origin,'/api/v1/session/login','POST',{'email':email,'password':password},headers); self.assertEqual(status,200)
            cookie=response_headers['set-cookie'].split(';',1)[0]; auth={**headers,'Cookie':cookie,'X-Canopy-CSRF':grant['csrf_token']}
            status,new_headers,new_grant=call(origin,'/api/v1/session/renew','POST',{},auth); self.assertEqual(status,200)
            new_cookie=new_headers['set-cookie'].split(';',1)[0]; self.assertNotEqual(cookie,new_cookie)
            self.assertEqual(call(origin,'/api/v1/audit',headers={'Cookie':cookie})[0],401)
            audit=call(origin,'/api/v1/audit',headers={'Cookie':new_cookie})[2]; self.assertTrue(any(e['action']=='owner.password_hash_upgraded' for e in audit['events']))
            self.assertEqual(call(origin,'/api/v1/session/logout','POST',{}, {**headers,'Cookie':new_cookie,'X-Canopy-CSRF':new_grant['csrf_token']})[0],204)
            self.assertEqual(call(origin,'/api/v1/audit',headers={'Cookie':new_cookie})[0],401)
            oversized={'email':email,'password':'x'*20000}; self.assertEqual(call(origin,'/api/v1/session/login','POST',oversized,headers)[0],413)
            for attempt in range(3): last=call(origin,'/api/v1/session/login','POST',{'email':email,'password':'definitely wrong'},headers)[0]
            self.assertEqual(last,429); self.assertEqual(call(origin,'/api/v1/session/login','POST',{'email':email,'password':password},headers)[0],429)
            logs+=daemon.stop(); self.assertNotIn(password,logs); self.assertNotIn(recovery,logs); self.assertNotIn(key_bytes.hex(),logs)

            state2=root/'expiry-state'; expiring=Daemon(state2,key,WORKFLOWD_SESSION_TTL_SECONDS='1',WORKFLOWD_LOGIN_MAX_FAILURES='100'); self.addCleanup(expiring.stop); eh={'Origin':expiring.origin}
            call(expiring.origin,'/api/v1/setup','POST',{'email':email,'password':password,'recovery_passphrase':recovery},eh)
            _,expiry_headers,_=call(expiring.origin,'/api/v1/session/login','POST',{'email':email,'password':password},eh); expiry_cookie=expiry_headers['set-cookie'].split(';',1)[0]
            time.sleep(1.1); self.assertEqual(call(expiring.origin,'/api/v1/audit',headers={'Cookie':expiry_cookie})[0],401); expiring.stop()

if __name__=='__main__': unittest.main()
