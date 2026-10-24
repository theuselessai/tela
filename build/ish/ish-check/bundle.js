// Offline checks for this updated Tela artifact, not a network or pairing app.
var initialState={status:'Checking updated runtime…',ok:false,busy:false};
var keybindings={normal:{q:'quit',Enter:'benchmark'}};
function hex(b){return Array.from(b,x=>x.toString(16).padStart(2,'0')).join('');}
function reduce(s,a){
 if(a.type==='result')return {status:a.status,ok:a.ok,busy:false};
 if(a.type==='benchmark'&&s.ok&&!s.busy){
  var start=Date.now();
  try{
   var job=Tela.crypto.argon2id(new TextEncoder().encode('tela-artifact-check'),new Uint8Array([0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]),{version:19,memoryKiB:65536,passes:3,parallelism:1,outputLength:80});
   job.result.then(function(bytes){var elapsed=Date.now()-start;var ok=bytes.length===80;bytes.fill(0);Tela.dispatch({type:'result',ok:ok,status:ok?'Argon2id 64 MiB / 3 passes completed in '+elapsed+' ms. Record this timing for the device test.':'Argon2 result failed'});},function(){Tela.dispatch({type:'result',ok:false,status:'Argon2 check failed. Stop here and report the error.'});});
  }catch(e){Tela.dispatch({type:'result',ok:false,status:'Could not start Argon2: '+String(e.message)});}
  return {status:'Argon2id running (64 MiB, 3 passes). Please let it finish; cancellation does not interrupt native computation.',ok:true,busy:true};
 }
 return s;
}
function view(s){return h('layout',{direction:'vertical'},h('text',{flex:1},'Tela iSH artifact check — offline\n\n'+s.status),h('text',{height:2},s.busy?'Working…':'Enter: optional 64 MiB Argon2 timing · q: quit\nNo network, credential storage or host connection.'));}
setTimeout(function(){
 var a,b,key;
 try{
  var c=Tela.crypto;if(!c)throw Error('Missing generic crypto API');
  if(hex(c.sha256(new Uint8Array([97,98,99])))!=='ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad')throw Error('SHA-256 mismatch');
  var encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
  if(decoder.decode(encoder.encode('café 日本語'))!=='café 日本語')throw Error('UTF-8 mismatch');
  a=c.x25519.generate();b=c.x25519.generate();
  var x=c.x25519.derive(a,c.x25519.publicKey(b)),y=c.x25519.derive(b,c.x25519.publicKey(a));
  if(!c.timingSafeEqual(x,y))throw Error('X25519 mismatch');x.fill(0);y.fill(0);
  var raw=c.randomBytes(32);key=c.chacha20poly1305.importKey32(raw);raw.fill(0);
  var nonce=new Uint8Array(12),aad=encoder.encode('fixture'),plain=encoder.encode('offline check');
  var ct=c.chacha20poly1305.seal(key,nonce,aad,plain),out=c.chacha20poly1305.open(key,nonce,aad,ct);
  if(!c.timingSafeEqual(plain,out))throw Error('AEAD mismatch');out.fill(0);
  Tela.dispatch({type:'result',ok:true,status:'PASS: QuickJS/UI, UTF-8, secure randomness, SHA-256, X25519 and ChaCha20-Poly1305.\nYou may run the optional Argon2 timing next.'});
 }catch(e){Tela.dispatch({type:'result',ok:false,status:'Check failed: '+String(e.message)});}
 finally{if(a)a.dispose();if(b)b.dispose();if(key)key.dispose();}
},1);
