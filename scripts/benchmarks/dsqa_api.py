"""Explicit DeepSearchQA autorater adapter; prompt supplied from the publication."""
import json
import os
from pathlib import Path
import sys
import urllib.error
import urllib.parse
import urllib.request


def metrics(judgment, answer_type="Set Answer"):
    if judgment is None:
        return dict.fromkeys(("precision","recall","f1","complete"))
    correctness=judgment["Answer Correctness"]
    details=correctness["Correctness Details"]
    excess=correctness["Excessive Answers"]
    if not details or any(type(v) is not bool for v in details.values()):
        raise ValueError("autorater must return a boolean for every expected answer")
    if not isinstance(excess,list) or any(not isinstance(v,str) for v in excess):
        raise ValueError("autorater excess answers must be a list of strings")
    tp=sum(details.values())
    precision=tp/(tp+len(excess)) if tp+len(excess) else 0
    recall=tp/len(details)
    complete=float(all(details.values()) and not excess)
    f1=2*precision*recall/(precision+recall) if precision+recall else 0
    if answer_type == "Single Answer":
        f1=complete
    return dict(precision=precision,recall=recall,f1=f1,complete=complete)


def main():
    task_file,answer_file,prompt_file,output,model=sys.argv[1:]
    tasks=json.loads(Path(task_file).read_text())
    answers=json.loads(Path(answer_file).read_text())
    template=Path(prompt_file).read_text()
    for name in ("prompt","prompt_type","answer","response"):
        if "{"+name+"}" not in template:
            raise ValueError(f"published autorater template missing {{{name}}}")
    key=os.environ.get("GEMINI_API_KEY")
    if not key:
        raise ValueError("native autorater needs GEMINI_API_KEY in its runtime environment")
    results=[]
    for t in tasks:
        prompt=template.format(prompt=t["prompt"],prompt_type=t["reference"]["answer_type"],
            answer=t["reference"]["answer"],response=answers[t["id"]])
        payload=dict(contents=[dict(parts=[dict(text=prompt)])],generationConfig=dict(responseMimeType="application/json"))
        request=urllib.request.Request("https://generativelanguage.googleapis.com/v1beta/models/"+urllib.parse.quote(model,safe="")+":generateContent",
            data=json.dumps(payload).encode(),headers={"Content-Type":"application/json","x-goog-api-key":key})
        try:
            with urllib.request.urlopen(request,timeout=600) as response:
                raw=json.load(response)
        except urllib.error.HTTPError as error:
            raise RuntimeError(f"Gemini autorater HTTP {error.code}; no automatic retry") from None
        text="".join(p.get("text","") for p in raw["candidates"][0]["content"]["parts"] if not p.get("thought"))
        judgment=json.loads(text)
        results.append(dict(id=t["id"],metrics=metrics(judgment,t["reference"]["answer_type"]),judgment=judgment,usage=raw.get("usageMetadata")))
        Path(output).write_text(json.dumps(results,indent=2,allow_nan=False)+"\n")


if __name__=="__main__":
    main()
