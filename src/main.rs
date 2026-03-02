pub(crate) mod parsing;
pub(crate) mod types;
pub(crate) mod xpath;

use std::path::Path;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ErrorCode, ServerCapabilities, ServerInfo};
use rmcp::transport::stdio;
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler, ServiceExt};
use serde::{Deserialize, Serialize};

use crate::types::XPathForRegexResponse;

#[tokio::main]
async fn main() {
    let service = HtmlRegexXpathTool::new().serve(stdio()).await.unwrap();
    service.waiting().await.unwrap();
}

#[derive(Debug, Clone)]
pub struct HtmlRegexXpathTool {
    pub tool_router: ToolRouter<Self>,
}

#[tool_router]
impl HtmlRegexXpathTool {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Provides a list of xpaths for strings matching a regex in an HTML or XML file")]
    pub fn xpath_for_regex(
        &self,
        Parameters(XPathForRegexRequest { file, regex }): Parameters<XPathForRegexRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let file_path = Path::new(&file);
        if !file_path.exists() {
            return Err(ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("File not found: {file}"),
                None,
            ));
        }

        let compiled_regex = regex::Regex::new(&regex).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("Invalid regex pattern: {e}"),
                None,
            )
        })?;

        let root = parsing::parse_file(file_path).map_err(|e| {
            ErrorData::new(ErrorCode::INTERNAL_ERROR, e, None)
        })?;

        let matches = xpath::xpath_for_regex(&root, &compiled_regex);

        let response = XPathForRegexResponse {
            file,
            regex,
            matches,
        };

        let json = serde_json::to_string_pretty(&response).map_err(|e| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to serialize response: {e}"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Evaluates an XPath expression against an HTML or XML file and returns the matching content")]
    pub fn evaluate_xpath(
        &self,
        Parameters(EvaluateXPathRequest { file, xpath }): Parameters<EvaluateXPathRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let file_path = Path::new(&file);
        if !file_path.exists() {
            return Err(ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("File not found: {file}"),
                None,
            ));
        }

        let root = parsing::parse_file(file_path).map_err(|e| {
            ErrorData::new(ErrorCode::INTERNAL_ERROR, e, None)
        })?;

        let results = xpath::evaluate_xpath(&root, &xpath).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("Invalid XPath expression: {e}"),
                None,
            )
        })?;

        let response = EvaluateXPathResponse {
            file,
            xpath,
            results,
        };

        let json = serde_json::to_string_pretty(&response).map_err(|e| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to serialize response: {e}"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct XPathForRegexRequest {
    #[schemars(description = "The html or xml file to read")]
    pub file: String,
    #[schemars(description = "The regex to search, and the results will be translated back to xpaths")]
    pub regex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EvaluateXPathRequest {
    #[schemars(description = "The html or xml file to read")]
    pub file: String,
    #[schemars(description = "The XPath expression to evaluate")]
    pub xpath: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvaluateXPathResponse {
    pub file: String,
    pub xpath: String,
    pub results: Vec<String>,
}

#[tool_handler]
impl ServerHandler for HtmlRegexXpathTool {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Provides regex to xpath translation for xml and html documents".to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
