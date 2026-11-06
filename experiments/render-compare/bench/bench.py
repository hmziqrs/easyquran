import subprocess,time,json,re,sys,statistics,os
O=os.path.dirname(os.path.abspath(__file__))
E='/Users/hmziq/os/easyquran/experiments'
APPS={'nsh':E+'/nsh-quran-sample/.nsh/out/nsh-quran-sample.app/Contents/MacOS/nshquransample','rune':E+'/rune-quran-sample/target/RuneQuran.app/Contents/MacOS/rune-quran-sample'}
def cpu(pid):
    t=subprocess.check_output(['ps','-o','time=','-p',str(pid)],text=True).strip()
    p=[float(x) for x in t.split(':')]; s=0
    for x in p: s=s*60+x
    return s
def foot(pid):
    o=subprocess.run(['footprint','-p',str(pid)],capture_output=True,text=True).stdout
    m=re.search(r'phys_footprint:\s*([\d.]+)\s*(KB|MB|GB)',o) or re.search(r'Footprint:\s*([\d.]+)\s*(KB|MB|GB)',o)
    if m: return float(m[1])*{'KB':1/1024,'MB':1,'GB':1024}[m[2]]
    return float(subprocess.check_output(['ps','-o','rss=','-p',str(pid)],text=True))/1024
def one(app,rep,scroll):
    env=dict(os.environ,QURAN_BENCH_REPEAT=str(rep),QURAN_BENCH_CYCLES='20')
    t0=time.time()
    o=subprocess.check_output([O+'/win',APPS[app]],env=env,text=True).split()
    pid=int(o[0]); r={'launch_ms':float(o[1])}; x,y,w,h=map(float,o[2:6])
    time.sleep(max(0,2.5-(time.time()-t0)))
    c1=cpu(pid); r['startup_cpu_s']=c1
    time.sleep(2.0); c2=cpu(pid); r['idle_cpu_pct']=(c2-c1)/2*100; r['idle_mb']=foot(pid)
    time.sleep(max(0,4.9-(time.time()-t0))); c3=cpu(pid)
    time.sleep(max(0,15.6-(time.time()-t0))); c4=cpu(pid)
    r['switch_cpu_ms']=(c4-c3)/20*1000; r['after_switch_mb']=foot(pid)
    if scroll:
        c5=cpu(pid); s=time.time()
        subprocess.run([O+'/scroll',str(x+w/2),str(y+h/2)]); r['scroll_s']=time.time()-s
        time.sleep(0.3); c6=cpu(pid); r['scroll_cpu_ms_per_s']=(c6-c5)*1000/(time.time()-s); r['after_scroll_mb']=foot(pid)
    subprocess.run(['kill',str(pid)]); time.sleep(1)
    return r
runs=int(sys.argv[1]); reps=[int(v) for v in sys.argv[2].split(',')]; res=[]
for rep in reps:
    for i in range(runs+1):
        for app in (['nsh','rune'] if i%2==0 else ['rune','nsh']):
            r=one(app,rep,rep>1); r.update(app=app,rep=rep,run=i); res.append(r); print(json.dumps(r),flush=True)
json.dump(res,open(sys.argv[3],'w'),indent=1)
