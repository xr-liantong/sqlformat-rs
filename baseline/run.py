import subprocess,time,resource,sys
args=sys.argv[1:]
t=time.monotonic()
r=subprocess.run(args,stdout=open('out.sql','wb'),stderr=subprocess.PIPE)
w=time.monotonic()-t
ru=resource.getrusage(resource.RUSAGE_CHILDREN)
print("wall=%.2fs rss=%dKB rc=%d out=%d err=%s"%(w,ru.ru_maxrss,r.returncode,__import__('os').path.getsize('out.sql'),r.stderr[:100]))
