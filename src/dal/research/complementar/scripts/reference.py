"""Refit the archived XGBoost specification on 2023–24 under its original encoding."""
from benchmark import *
import sys
sys.path.insert(0,str(ENGINE))
from src.domain.services.revision_study import encode
thai,ext=dataset()
train=thai[thai.parsed_date.dt.year<=2024]
test=thai[thai.parsed_date.dt.year==2025]
a,b,_=encode(train[FEATURES],test[FEATURES])
classes=sorted(thai.rice_stage.unique())
y=train.rice_stage.map(dict(zip(classes,range(6))))
model=XGBClassifier(n_estimators=350,learning_rate=.06,max_depth=6,tree_method='hist',random_state=43,eval_metric='mlogloss',n_jobs=4)
model.fit(a,y)
p=np.array(classes)[model.predict(b)]
out=test[['source_row_id','label']].copy()
out['pred']=[LABEL[v] for v in p]
out.to_csv(OUT/'archived_spec_forward.csv',index=False)
dump('archived_spec_forward.json',dict(arm='Thai only, original XGBoost recipe refitted, seed 43',macro_f1=float(f1_score(out.label,out.pred,labels=CLASSES,average='macro')),accuracy=float(accuracy_score(out.label,out.pred))))
