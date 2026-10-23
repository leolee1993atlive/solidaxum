use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct LoginBody {
    #[validate(length(min = 1, message = "clientId不能为空"))]
    pub client_id: String,
    #[validate(length(min = 1, message = "grantType不能为空"))]
    pub grant_type: String,
    pub code: Option<String>,
    pub uuid: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LoginVo {
    pub access_token: String,
    pub expire_in: u64,
    pub client_id: String,
    pub refresh_token: Option<String>,
    pub refresh_expire_in: Option<u64>,
    pub scope: Option<String>,
    pub openid: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RegisterBody {
    #[validate(length(min = 1, message = "用户名不能为空"))]
    pub username: String,
    #[validate(length(min = 5, max = 20, message = "密码长度5-20位"))]
    pub password: String,
    pub confirm_password: Option<String>,
    pub code: Option<String>,
    pub uuid: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptchaVo {
    pub captcha_enabled: bool,
    pub uuid: Option<String>,
    pub img: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_body_deserializes_camel_case() {
        let json = r#"{
            "clientId": "e5cd7e4891bf95d1d19206ce24a7b32e",
            "grantType": "password",
            "code": "8",
            "uuid": "abc123",
            "username": "admin",
            "password": "admin123"
        }"#;
        let body: LoginBody = serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(body.client_id, "e5cd7e4891bf95d1d19206ce24a7b32e");
        assert_eq!(body.grant_type, "password");
        assert_eq!(body.code.as_deref(), Some("8"));
        assert_eq!(body.username.as_deref(), Some("admin"));
        assert_eq!(body.password.as_deref(), Some("admin123"));
    }

    #[test]
    fn test_captcha_vo_serializes_camel_case() {
        let vo = CaptchaVo {
            captcha_enabled: true,
            uuid: Some("abc".to_string()),
            img: Some("iVBORw0KGgoAAAANS".to_string()),
        };
        let json = serde_json::to_string(&vo).expect("序列化失败");
        assert!(
            json.contains("\"captchaEnabled\""),
            "应包含 captchaEnabled 字段, got: {json}"
        );
        assert!(json.contains("\"uuid\""));
        assert!(json.contains("\"img\""));
    }

    #[test]
    fn test_captcha_vo_disabled() {
        let vo = CaptchaVo {
            captcha_enabled: false,
            uuid: None,
            img: None,
        };
        let json = serde_json::to_string(&vo).expect("序列化失败");
        assert!(json.contains("\"captchaEnabled\":false"));
        assert!(json.contains("\"uuid\":null"));
        assert!(json.contains("\"img\":null"));
    }

    #[test]
    fn test_login_vo_serializes_snake_case() {
        let vo = LoginVo {
            access_token: "token123".to_string(),
            expire_in: 604800,
            client_id: "client456".to_string(),
            refresh_token: None,
            refresh_expire_in: None,
            scope: None,
            openid: None,
        };
        let json = serde_json::to_string(&vo).expect("序列化失败");
        assert!(
            json.contains("\"access_token\""),
            "应包含 access_token 字段, got: {json}"
        );
        assert!(json.contains("\"expire_in\""));
        assert!(json.contains("\"client_id\""));
    }

    #[test]
    fn test_register_body_deserializes_camel_case() {
        let json = r#"{
            "username": "newuser",
            "password": "pass123",
            "confirmPassword": "pass123",
            "code": "8",
            "uuid": "abc123"
        }"#;
        let body: RegisterBody =
            serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(body.username, "newuser");
        assert_eq!(body.password, "pass123");
        assert_eq!(body.confirm_password.as_deref(), Some("pass123"));
    }
}
