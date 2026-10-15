//! Count the exact pretty JSON plus newline used by the revision writer, without
//! allocating another serialized frame. Stop as soon as the read bound is exceeded.

struct Remaining(usize);

impl std::io::Write for Remaining {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| std::io::Error::other("frame size bound"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) fn fits(frame: &serde_json::Value, maximum: usize) -> bool {
    let mut remaining_values = crate::residual::MAX_JSON_VALUES;
    if !shape(frame, 0, &mut remaining_values) {
        return false;
    }
    let Some(remaining) = maximum.checked_sub(1) else {
        return false;
    };
    serde_json::to_writer_pretty(Remaining(remaining), frame).is_ok()
}

fn shape(value: &serde_json::Value, depth: usize, remaining: &mut usize) -> bool {
    if *remaining == 0 || depth > crate::residual::MAX_JSON_DEPTH {
        return false;
    }
    *remaining -= 1;
    match value {
        serde_json::Value::Array(items) => {
            items.iter().all(|item| shape(item, depth + 1, remaining))
        }
        serde_json::Value::Object(items) => {
            items.values().all(|item| shape(item, depth + 1, remaining))
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn exact_pretty_utf8_escaping_and_final_newline() {
        for frame in [
            serde_json::json!({}),
            serde_json::json!({"consts":[{"value":"é\n\"\\"}]}),
        ] {
            let bytes = serde_json::to_vec_pretty(&frame).unwrap().len() + 1;
            assert!(super::fits(&frame, bytes));
            assert!(!super::fits(&frame, bytes - 1));
            assert!(!super::fits(&frame, 0));
        }
    }

    #[test]
    fn retained_frame_respects_receipt_value_and_depth_bounds() {
        let mut frame = serde_json::Value::Array(vec![
            serde_json::Value::Null;
            crate::residual::MAX_JSON_VALUES - 1
        ]);
        assert!(super::fits(
            &frame,
            crate::residual::binding::MAX_BOUND_ARTIFACT_BYTES
        ));
        frame.as_array_mut().unwrap().push(serde_json::Value::Null);
        assert!(!super::fits(
            &frame,
            crate::residual::binding::MAX_BOUND_ARTIFACT_BYTES
        ));
        let mut deep = serde_json::Value::Null;
        for _ in 0..crate::residual::MAX_JSON_DEPTH {
            deep = serde_json::json!([deep]);
        }
        assert!(super::fits(&deep, 16384));
        assert!(!super::fits(&serde_json::json!([deep]), 16384));
    }
}
