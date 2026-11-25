"""Export all frozen CatBoost arms and verify complete 2025 prediction equivalence."""
import json
import joblib
import numpy as np
import pandas as pd
import onnxruntime as ort
from benchmark import OUT,CLASSES,THDIR,CATS,FEATURES,ARMS,train,dataset,dump

thai,ext=dataset()
dev=thai[thai.parsed_date.dt.year<=2024]
test=thai[thai.parsed_date.dt.year==2025]
reference=pd.read_csv(OUT/'forward_predictions.csv.gz')
for arm in ARMS:
    name=arm+'__cat'
    pipe=train(dev,ext,arm,'cat')
    prep=pipe['prepare']
    clf=pipe['model']
    clf.save_model(str(OUT/(name+'.onnx')),format='onnx')
    x=prep.transform(test[FEATURES]).astype('float32')
    options=ort.SessionOptions();options.intra_op_num_threads=1;options.inter_op_num_threads=1
    sess=ort.InferenceSession(str(OUT/(name+'.onnx')),options,providers=['CPUExecutionProvider'])
    res=sess.run(None,{sess.get_inputs()[0].name:x})
    scores=np.array([[r[i] for i in range(6)] for r in res[1]])
    expected=reference[[name+'_p_'+cls for cls in CLASSES]].to_numpy()
    error=float(np.abs(scores-expected).max())
    assert error<1e-5 and np.array_equal(scores.argmax(axis=1),reference[name].to_numpy())
    meta=dict(classes=CLASSES,numeric=[k for k in FEATURES if k not in CATS],categories={k:cats.tolist() for k,cats in zip(CATS,prep.named_transformers_['cat'].categories_)},category_order=CATS,unknown='all zeroes in its one-hot block',input_dtype='float32',equivalence_rows=len(x),max_probability_error=error)
    dump(name+'_schema.json',meta)
    np.save(OUT/(name+'_fixture.npy'),x[::13].copy())
