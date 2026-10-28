//! Regression tests for actionable errors from mock Evernote responses.

use super::*;
use std::io::Cursor;
use thrift::protocol::{
    TFieldIdentifier, TMessageIdentifier, TMessageType, TOutputProtocol, TSerializable,
    TStructIdentifier, TType,
};

/// Maps a typed SDK exception without making an API request.
fn user_error(code: EDAMErrorCode, parameter: Option<&str>) -> String {
    map_thrift_error(EDAMUserException::new(code, parameter.map(str::to_owned)).into()).to_string()
}

#[test]
fn expired_token_explains_the_error_and_how_to_replace_the_token() {
    let message = user_error(EDAMErrorCode::AUTH_EXPIRED, Some("authenticationToken"));
    assert_eq!(
        message,
        "Evernote API error: AUTH_EXPIRED (9) (parameter: \"authenticationToken\"): \
         authentication token has expired or been revoked.\n\
         Run `reeknote login` to authorize again. For developer tokens, obtain a new token first:\n\
         https://www.evernote.com/api/DeveloperToken.action\n\
         If EVERNOTE_DEV_TOKEN is set, update or unset it; it overrides the saved token."
    );
}

#[test]
fn expired_token_without_a_parameter_still_has_recovery_guidance() {
    for parameter in [None, Some("")] {
        let message = user_error(EDAMErrorCode::AUTH_EXPIRED, parameter);
        assert!(message.contains("AUTH_EXPIRED (9): authentication token has expired"));
        assert!(message.contains("reeknote login"));
        assert!(message.contains("https://www.evernote.com/api/DeveloperToken.action"));
        assert!(!message.contains("parameter:"));
    }
}

#[test]
fn invalid_auth_is_distinct_from_an_expired_token() {
    let message = user_error(EDAMErrorCode::INVALID_AUTH, Some("authenticationToken"));
    assert!(message.contains("INVALID_AUTH (8)"));
    assert!(message.contains("credentials were rejected"));
    assert!(message.contains("reeknote login"));
    assert!(message.contains("EVERNOTE_DEV_TOKEN"));
    assert!(!message.contains("expired"));
}

#[test]
fn unrelated_user_errors_preserve_details_without_reauthorization_advice() {
    for (code, parameter, expected) in [
        (
            EDAMErrorCode::PERMISSION_DENIED,
            Some("Note.content"),
            "Evernote API error: PERMISSION_DENIED (3) (parameter: \"Note.content\")",
        ),
        (
            EDAMErrorCode::QUOTA_REACHED,
            None,
            "Evernote API error: QUOTA_REACHED (7)",
        ),
        (
            EDAMErrorCode::BAD_DATA_FORMAT,
            Some("authenticationToken"),
            "Evernote API error: BAD_DATA_FORMAT (2) (parameter: \"authenticationToken\")",
        ),
        (
            EDAMErrorCode::DATA_REQUIRED,
            Some("authenticationToken"),
            "Evernote API error: DATA_REQUIRED (5) (parameter: \"authenticationToken\")",
        ),
    ] {
        assert_eq!(user_error(code, parameter), expected);
    }
}

#[test]
fn unknown_future_error_codes_keep_their_numeric_value() {
    assert_eq!(
        user_error(EDAMErrorCode(12345), None),
        "Evernote API error: UNRECOGNIZED_ERROR (12345)"
    );
}

#[test]
fn parameter_control_characters_are_escaped() {
    let message = user_error(EDAMErrorCode::BAD_DATA_FORMAT, Some("Note.\n\x1b[31m"));
    assert!(message.contains(r"Note.\n\u{1b}[31m"));
    assert!(!message.contains('\n'));
    assert!(!message.contains('\x1b'));
}

#[test]
fn other_thrift_errors_keep_their_original_message() {
    let errors = [
        thrift::Error::from(io::Error::new(io::ErrorKind::TimedOut, "request timed out")),
        thrift::Error::Protocol(thrift::ProtocolError::new(
            thrift::ProtocolErrorKind::InvalidData,
            "invalid reply",
        )),
        thrift::Error::Application(thrift::ApplicationError::new(
            thrift::ApplicationErrorKind::UnknownMethod,
            "unknown method",
        )),
        thrift::Error::User(Box::new(io::Error::other("unrecognized service exception"))),
    ];
    for error in errors {
        let expected = format!("Evernote API error: {error}");
        assert_eq!(map_thrift_error(error).to_string(), expected);
    }
}

#[test]
fn expired_token_from_a_binary_thrift_reply_reaches_the_error_mapper() {
    let exception = EDAMUserException::new(
        EDAMErrorCode::AUTH_EXPIRED,
        Some("authenticationToken".to_string()),
    );
    let response = get_user_error_reply(&exception);
    let mut client = UserStoreSyncClient::new(
        TBinaryInputProtocol::new(Cursor::new(response), true),
        TBinaryOutputProtocol::new(io::sink(), true),
    );

    let error = client
        .get_user("mock-secret-token".to_string())
        .unwrap_err();
    let message = map_thrift_error(error).to_string();

    assert!(message.contains("AUTH_EXPIRED (9)"));
    assert!(message.contains("parameter: \"authenticationToken\""));
    assert!(message.contains("reeknote login"));
    assert!(message.contains("https://www.evernote.com/api/DeveloperToken.action"));
    assert!(!message.contains("mock-secret-token"));
}

/// Encodes a UserStore reply using the SDK's serializer and the real Thrift codec.
fn get_user_error_reply(exception: &EDAMUserException) -> Vec<u8> {
    let mut response = Vec::new();
    let mut protocol = TBinaryOutputProtocol::new(&mut response, true);
    protocol
        .write_message_begin(&TMessageIdentifier::new("getUser", TMessageType::Reply, 1))
        .unwrap();
    protocol
        .write_struct_begin(&TStructIdentifier::new("getUser_result"))
        .unwrap();
    protocol
        .write_field_begin(&TFieldIdentifier::new("userException", TType::Struct, 1))
        .unwrap();
    exception.write_to_out_protocol(&mut protocol).unwrap();
    protocol.write_field_end().unwrap();
    protocol.write_field_stop().unwrap();
    protocol.write_struct_end().unwrap();
    protocol.write_message_end().unwrap();
    protocol.flush().unwrap();
    response
}
