interface DataStreamProps {
  lines: string[];
  className?: string;
}

function joinClasses(...classes: Array<string | undefined>): string {
  return classes.filter((value) => value && value.length > 0).join(" ");
}

export function DataStream({ lines, className }: DataStreamProps): JSX.Element {
  const source = lines.length > 0 ? lines : ["[stream] awaiting events..."];
  const repeated = [...source, ...source];

  return (
    <div className={joinClasses("data-stream", className)}>
      <div className="data-stream__track">
        {repeated.map((line, index) => (
          <p key={`${index}-${line}`} className="data-stream__line">
            {line}
          </p>
        ))}
      </div>
    </div>
  );
}
