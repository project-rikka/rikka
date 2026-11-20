import c from "workspace:sdk/examples/c/counter.c";
import cpp from "workspace:sdk/examples/cpp/counter.cpp";
import csharp from "workspace:sdk/examples/csharp/Counter.cs";
import go from "workspace:sdk/examples/go/counter/main.go";
import java from "workspace:sdk/examples/java/Counter.java";
import python from "workspace:sdk/examples/python/counter.py";

export interface Example {
  readonly language: string;
  readonly grammar: "c" | "cpp" | "go" | "csharp" | "java" | "python";
  readonly path: string;
  readonly source: string;
}

export const examples: readonly Example[] = [
  {
    language: "C",
    grammar: "c",
    path: "sdk/examples/c/counter.c",
    source: c,
  },
  {
    language: "C++",
    grammar: "cpp",
    path: "sdk/examples/cpp/counter.cpp",
    source: cpp,
  },
  {
    language: "Go",
    grammar: "go",
    path: "sdk/examples/go/counter/main.go",
    source: go,
  },
  {
    language: "C#",
    grammar: "csharp",
    path: "sdk/examples/csharp/Counter.cs",
    source: csharp,
  },
  {
    language: "Java",
    grammar: "java",
    path: "sdk/examples/java/Counter.java",
    source: java,
  },
  {
    language: "Python",
    grammar: "python",
    path: "sdk/examples/python/counter.py",
    source: python,
  },
];
