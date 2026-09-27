"""Convert saved prose into cited sentences without rewriting the answer."""
import json
from pathlib import Path
from .data import read, write, file_id, rows
from .runner import read_evaluation, assess
from .upstream import jsonl


def sentences(answer, result):
    responses=result["responses"]
    offset=0
    for sentence in responses:
        text=sentence["text"]
        if not text:
            raise ValueError("empty sentence in report conversion")
        start=answer.find(text,offset)
        if start<0 or answer[offset:start].strip():
            raise ValueError("conversion changed or omitted answer text")
        offset=start+len(text)
        for citation in sentence["citations"]:
            if not isinstance(citation,str) or citation not in answer:
                raise ValueError("conversion introduced a citation absent from the answer")
    if answer[offset:].strip():
        raise ValueError("conversion omitted the end of the answer")
    return responses


def run(args):
    evaluation=read_evaluation(args.evaluation)
    output=Path(args.output)
    output.mkdir(parents=True,exist_ok=False)
    corpus={str(r["id"]):r["text"] for r in rows(args.corpus)} if args.corpus else None
    reports=[]
    for m in evaluation["measurements"]:
        if m["arm"] != args.arm: continue
        rid=file_id(m["research_id"])
        answer=(Path(args.evaluation).parent/f"{rid}.md").read_text()
        result=assess(args.binary,dict(objective='Split the supplied saved answer into sentences and identify its cited document IDs. This is format conversion, not research or editing. Preserve ALL characters in text, including headings, Markdown and citation markers; only whitespace between segments may be omitted. Do not repair, add or remove claims or citations. Resolve footnote citations only from this answer. Return JSON in draft: {"responses":[{"text":"exact substring of answer","citations":["document ID exactly present in answer"]}]}. Do not use tools. Finish.',
            context=answer,seconds=args.seconds,tool_calls=0),output/rid)
        responses=sentences(answer,result)
        references=list(dict.fromkeys(c for s in responses for c in s["citations"]))
        metadata=dict(team_id="mcp-deepresearch",run_id=rid,topic_id=m["case"])
        reports.append(dict(metadata=metadata,responses=responses,references=references))
    if not reports:
        raise ValueError("selected arm has no saved reports")
    jsonl(output/"argue-reports.jsonl",reports)
    trec=[]
    for r in reports:
        metadata={**r["metadata"],"narrative_id":r["metadata"]["topic_id"],"type":"automatic"}
        item=dict(metadata=metadata,topic_id=metadata["topic_id"],run_id=metadata["run_id"],references=r["references"],
            answer=[dict(text=s["text"],citations=[r["references"].index(c) for c in s["citations"]]) for s in r["responses"]])
        if corpus is not None:
            missing=set(r["references"])-corpus.keys()
            if missing:
                raise ValueError(f"cited IDs missing from supplied corpus: {sorted(missing)}")
            item["segments"]={c:corpus[c] for c in r["references"]}
        trec.append(item)
    jsonl(output/"trec-answers.jsonl",trec)
    write(output/"conversion.json",dict(note="Model sentence segmentation is an adaptation. Text is checked for preservation; citation association still requires review.",
        source_evaluation=str(Path(args.evaluation).resolve()),arm=args.arm))
