//! The typed reasons of `parse_trace` a reader decides on (wave c1, correction 2 item 10), and
//! the one tokenizer of the grammar, `trace_lines`: each reason keeps the message the parser
//! gave before it had a type of its own.

use patchback_fit::{TraceLine, TraceReason, parse_trace, trace_lines};

fn text(lines: &[&str]) -> String {
    let mut text = String::from("patchback-fit-trace 1\ntopology t/1 a b\n");
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str("completed ImprovementBelowThreshold\n");
    text
}

const FIRST: &str = "evaluation 1 1 0 first-retained t/1 0.8 0.6 0.4 0.5 0.645 0.645 0.5 0.5";
const SECOND: &str = "evaluation 2 2 0 not-improved t/1 0.9 0.7 0.5 0.6 0.745 0.645 0.5 0.5";

#[test]
fn a_sequence_that_is_not_the_next_names_itself_and_the_highest_held() {
    let repeated = SECOND.replacen("evaluation 2 2", "evaluation 1 1", 1);
    let error = parse_trace(&text(&[FIRST, &repeated])).unwrap_err();
    assert_eq!(error.line, Some(4));
    assert_eq!(
        error.reason,
        TraceReason::SequenceOutOfOrder {
            sequence: 1,
            held: 1
        }
    );
    assert_eq!(
        error.to_string(),
        "line 4: expected sequence numbers 1, 2, 3 and on, in order"
    );

    let gap = SECOND.replacen("evaluation 2 2", "evaluation 3 3", 1);
    let error = parse_trace(&text(&[FIRST, &gap])).unwrap_err();
    assert_eq!(
        error.reason,
        TraceReason::SequenceOutOfOrder {
            sequence: 3,
            held: 1
        }
    );

    let zero = FIRST.replacen("evaluation 1 1", "evaluation 0 0", 1);
    let error = parse_trace(&text(&[&zero])).unwrap_err();
    assert_eq!(
        error.reason,
        TraceReason::SequenceOutOfOrder {
            sequence: 0,
            held: 0
        }
    );
}

#[test]
fn an_evaluation_count_of_zero_is_its_own_reason() {
    let zero = FIRST.replacen("evaluation 1 1", "evaluation 1 0", 1);
    let error = parse_trace(&text(&[&zero])).unwrap_err();
    assert_eq!(
        error.reason,
        TraceReason::EvaluationCountZero { sequence: 1 }
    );
    assert_eq!(
        error.to_string(),
        "line 3: expected an evaluation count of at least 1"
    );
}

#[test]
fn a_rejected_line_with_a_loss_is_its_own_reason() {
    let rejected = "evaluation 2 2 0 rejected t/1 0.7 0.645 0.5 0.5";
    let error = parse_trace(&text(&[FIRST, rejected])).unwrap_err();
    assert_eq!(error.reason, TraceReason::RejectedWithLoss { sequence: 2 });
    assert_eq!(
        error.to_string(),
        "line 4: expected `-` for the losses of a rejected candidate"
    );
    // a rejected line of five losses has the wrong field count: still a malformed record
    let five = "evaluation 2 2 0 rejected t/1 0.9 0.7 0.5 0.6 0.745 0.645 0.5 0.5";
    let error = parse_trace(&text(&[FIRST, five])).unwrap_err();
    assert!(
        matches!(error.reason, TraceReason::MalformedRecord { .. }),
        "{error:?}"
    );
}

#[test]
fn trace_lines_is_the_parsers_tokenizer() {
    assert_eq!(trace_lines(""), Vec::<TraceLine<'_>>::new());
    assert_eq!(
        trace_lines("\n"),
        vec![TraceLine {
            line: 1,
            content: ""
        }]
    );
    let lines = trace_lines("a b\r\nevaluation\t1  2\nlast");
    assert_eq!(
        lines
            .iter()
            .map(|l| (l.line, l.content))
            .collect::<Vec<_>>(),
        [(1, "a b"), (2, "evaluation\t1  2"), (3, "last")]
    );
    assert_eq!(lines[1].fields(), ["evaluation", "1", "2"]);
    assert!(lines[1].is_evaluation());
    assert!(!lines[0].is_evaluation());

    // a trace whose fields are separated by tabs and runs of spaces parses as the spaced one
    let spaced = text(&[FIRST, SECOND]);
    let tabbed = spaced
        .replace("evaluation ", "evaluation\t")
        .replace(" t/1 ", "  t/1\t");
    assert_eq!(parse_trace(&tabbed), parse_trace(&spaced));
    assert!(parse_trace(&spaced).is_ok());
}
