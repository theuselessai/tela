#!/usr/bin/env python3
"""Synthetic loopback-TLS URL approval + stock engine in an owned PTY.
No public endpoints, user grants, real credentials or real-device approval.
"""
import fcntl, json, os, pathlib, pty, select, signal, struct, subprocess, termios, time
root=pathlib.Path(__file__).resolve().parents[1]
env=os.environ.copy()
if os.uname().sysname=='Darwin':
    env.setdefault('SDKROOT',subprocess.run(['xcrun','--show-sdk-path'],text=True,stdout=subprocess.PIPE,check=True).stdout.strip())
build=subprocess.run(['cargo','test','--offline','--locked','-p','tela-cli','--no-run','--message-format=json'],cwd=root,env=env,text=True,stdout=subprocess.PIPE,check=True)
bins=[r['executable'] for line in build.stdout.splitlines() if (r:=json.loads(line)).get('reason')=='compiler-artifact' and r.get('executable') and r.get('target',{}).get('name')=='tela' and r.get('profile',{}).get('test')]
assert len(bins)==1
pid,fd=pty.fork()
if pid==0:
    env['TELA_REMOTE_PTY_TEST']='1';env['TERM']='xterm-256color'
    os.execve(bins[0],[bins[0],'--ignored','--exact','remote::tls_tests::remote_approval_and_engine_on_pty','--nocapture'],env)
fcntl.ioctl(fd,termios.TIOCSWINSZ,struct.pack('HHHH',30,100,0,0))
output=b'';approved=False;enter_sent=False;quit_sent=False;status=None
try:
    deadline=time.monotonic()+15
    while time.monotonic()<deadline:
        if select.select([fd],[],[],.05)[0]:
            try:chunk=os.read(fd,4096)
            except OSError:chunk=b''
            output+=chunk
            if b'Run this code with these permissions?' in output and not approved:
                os.write(fd,b'yes\n');approved=True
            if b'URL_FIXTURE_READY' in output and not enter_sent:
                os.write(fd,b'\r');enter_sent=True
            # All cells differ from the initial marker, so ratatui emits one
            # contiguous marker rather than splitting it into cursor-positioned deltas.
            if b'111111111111111111111111' in output and not quit_sent:
                os.write(fd,b'q');quit_sent=True
        done,value=os.waitpid(pid,os.WNOHANG)
        if done:status=value;break
    assert status is not None,'URL/PTY fixture timed out: '+output.decode(errors='replace')
    assert os.waitstatus_to_exitcode(status)==0,output.decode(errors='replace')
    assert approved and enter_sent and quit_sent,output.decode(errors='replace')
    print('Loopback TLS URL approval + stock engine rendering: passed (synthetic only).')
finally:
    if status is None:
        os.kill(pid,signal.SIGKILL);os.waitpid(pid,0)
    os.close(fd)
