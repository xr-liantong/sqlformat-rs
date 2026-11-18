import random
random.seed(20261119)
tables=['users','orders','products','sessions','payments','events','invoices','shipments','accounts','tickets']
cols=['id','name','email','status','created_at','updated_at','amount','currency','region','channel','customer_id','total','quantity','price','sku','note','owner_id','score']
def stmt(i):
    t=random.choice(tables); c1,c2,c3,c4=random.sample(cols,4)
    return ("SELECT %s, %s, %s, %s FROM %s WHERE %s = 'v%d' AND %s > %d ORDER BY %s DESC LIMIT 100;"
            %(c1,c2,c3,c4,t,c1,i%89,c2,i%997,c3))
lines=[stmt(i) for i in range(20000)]
open('corpus.sql','w').write("\n".join(lines)+"\n")
