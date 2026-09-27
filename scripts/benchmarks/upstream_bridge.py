"""ResearchRubrics' public Python entrypoint, called in its prepared environment."""
import asyncio
import json
from pathlib import Path
import sys


async def main():
    checkout, tasks, reports, output = map(Path, sys.argv[1:])
    sys.path.insert(0, str(checkout/"src/evaluate_rubrics"))
    from evaluate_single_report import evaluate_task_rubrics
    results=[]
    for report in sorted(reports.glob("*.md")):
        frame, compliance = await evaluate_task_rubrics(str(report), str(tasks))
        succeeded=bool(frame["success"].all())
        results.append(dict(id=report.stem,compliance=float(compliance) if succeeded else None,
                            judgments=json.loads(frame.to_json(orient="records"))))
        output.write_text(json.dumps(results,indent=2,allow_nan=False)+"\n")


if __name__=="__main__":
    asyncio.run(main())
