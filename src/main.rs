use std::path::Path;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler, ServiceExt};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ErrorCode, ServerCapabilities, ServerInfo};
use rmcp::transport::stdio;
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    let service = HtmlRegexXpathTool::new().serve(stdio()).await.unwrap();
    service.waiting().await.unwrap();
}

#[derive(Debug, Clone)]
pub struct HtmlRegexXpathTool {
    pub tool_router: ToolRouter<Self>
}

#[tool_router]
impl HtmlRegexXpathTool {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router()
        }
    }

    #[tool(description="Provides a list of xpaths for strings matching a regex")]
    pub fn xpath_for_regex(&self, Parameters(XPathForRegexRequest { file, regex}): Parameters<XPathForRegexRequest>) -> Result<CallToolResult, ErrorData>{
        let file_path = Path::new(&file);
        if !file_path.exists() {
            return Err(ErrorData::new(ErrorCode::INVALID_REQUEST, "bla", None))
        }
        let mut content: Vec<Content> = Vec::new();
        content.push(Content::text("This would be an xpath for a regex result match".to_string()));
        Ok(CallToolResult::success(content))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct XPathForRegexRequest {
    #[schemars(description = "The html or xml file to read")]
    pub file: String,
    #[schemars(description = "The regex to search, and the results will be translated back to xpaths")]
    pub regex: String
}


#[tool_handler]
impl ServerHandler for HtmlRegexXpathTool {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Provides regex to xpath translation for xml and html documents".to_string()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}