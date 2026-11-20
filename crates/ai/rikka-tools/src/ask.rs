use rikka_agent::{Function, Reply};
use rikka_llm::{Schema, Tool, Value};
use rikka_prompt::{Facts, Prompt};
use serde::Deserialize;

use crate::answers::{AnsweredQuestion, Answers};
use crate::error::{Error, Result};
use crate::question::Question;
use crate::questions::Questions;

#[derive(Debug, Deserialize)]
struct Args {
    questions: Vec<Question>,
}

pub(crate) fn declare(questions: &Questions, prompt: &Prompt, facts: &Facts) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .param(
            "questions",
            Schema::array(questions_schema(prompt, facts)?)
                .describe(prompt.param("questions", facts)?),
        )
        .build()
        .map_err(|error| Error::Undeclarable {
            name: prompt.name().to_owned(),
            reason: error.to_string(),
        })?;

    let questions = questions.clone();
    Ok(Function::new(tool, move |input: Value| {
        let questions = questions.clone();
        async move { run(&questions, input).await }
    }))
}

fn questions_schema(prompt: &Prompt, facts: &Facts) -> Result<Schema> {
    let described = |name: &str| prompt.param(name, facts);

    Ok(Schema::object()
        .param(
            "question",
            Schema::string().describe(described("question")?),
        )
        .param(
            "options",
            Schema::array(
                Schema::object()
                    .param("label", Schema::string().describe(described("label")?))
                    .param(
                        "description",
                        Schema::string().describe(described("description")?),
                    ),
            )
            .describe(described("options")?),
        )
        .optional("header", Schema::string().describe(described("header")?))
        .optional(
            "multi_select",
            Schema::boolean().describe(described("multi_select")?),
        ))
}

async fn run(questions: &Questions, input: Value) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;

    if args.questions.is_empty() {
        return Err("no questions given".to_owned());
    }
    if let Some(error) = args.questions.iter().find_map(Question::validation_error) {
        return Err(format!("invalid question: {error}"));
    }

    let mut waiting = questions.put(args.questions.clone());
    let Some(selections) = waiting.answer().await else {
        return Err("question withdrawn before it was answered".to_owned());
    };

    let answers: Vec<AnsweredQuestion> = args
        .questions
        .iter()
        .zip(selections)
        .filter(|(_, selections)| !selections.is_empty())
        .map(|(question, selections)| AnsweredQuestion::new(question, selections))
        .collect();

    if answers.is_empty() {
        return Err("the person answered nothing".to_owned());
    }
    Ok(Reply::json(&Answers::new(answers)))
}
