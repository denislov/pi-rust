use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use pi_agent_core::api::flow::{Action, Flow, FlowError, FlowNode, FlowOutcome};

use super::{
    CodingAgentSessionExport, export_from_replay, render_export_html, write_rendered_export_html,
};
use crate::runtime::error::CodingSessionError;
use crate::runtime::facade::context::CodingAgentSessionSummary;
use crate::session::replay::SessionReplay;

const DEFAULT_ACTION: &str = "default";

pub(crate) const EXPORT_NODE_IDS: &[&str] = &[
    "start_export",
    "load_session_replay",
    "select_export_view",
    "render_export",
    "write_export",
    "emit_completion",
];

const EXPORT_NODE_SPECS: &[ExportNodeSpec] = &[
    ExportNodeSpec {
        id: "start_export",
        name: "StartExport",
        kind: ExportNodeKind::StartExport,
    },
    ExportNodeSpec {
        id: "load_session_replay",
        name: "LoadSessionReplay",
        kind: ExportNodeKind::LoadSessionReplay,
    },
    ExportNodeSpec {
        id: "select_export_view",
        name: "SelectExportView",
        kind: ExportNodeKind::SelectExportView,
    },
    ExportNodeSpec {
        id: "render_export",
        name: "RenderExport",
        kind: ExportNodeKind::RenderExport,
    },
    ExportNodeSpec {
        id: "write_export",
        name: "WriteExport",
        kind: ExportNodeKind::WriteExport,
    },
    ExportNodeSpec {
        id: "emit_completion",
        name: "EmitCompletion",
        kind: ExportNodeKind::EmitCompletion,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExportNodeSpec {
    id: &'static str,
    name: &'static str,
    kind: ExportNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportNodeKind {
    StartExport,
    LoadSessionReplay,
    SelectExportView,
    RenderExport,
    WriteExport,
    EmitCompletion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportOptions {
    target: ExportTarget,
}

impl ExportOptions {
    pub(crate) fn view() -> Self {
        Self {
            target: ExportTarget::View,
        }
    }

    pub(crate) fn html(path: impl Into<PathBuf>) -> Self {
        Self {
            target: ExportTarget::Html(path.into()),
        }
    }

    fn output_path(&self) -> Option<&Path> {
        match &self.target {
            ExportTarget::View => None,
            ExportTarget::Html(path) => Some(path.as_path()),
        }
    }

    fn writes_html(&self) -> bool {
        matches!(self.target, ExportTarget::Html(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExportTarget {
    View,
    Html(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportOutcome {
    pub(crate) export: CodingAgentSessionExport,
    pub(crate) path: Option<PathBuf>,
}

pub(crate) struct ExportContext {
    options: ExportOptions,
    summary: CodingAgentSessionSummary,
    replay: SessionReplay,
    export: Option<CodingAgentSessionExport>,
    rendered_html: Option<String>,
    written_path: Option<PathBuf>,
    failure_error: Option<CodingSessionError>,
}

impl ExportContext {
    pub(crate) fn new(
        options: ExportOptions,
        summary: CodingAgentSessionSummary,
        replay: SessionReplay,
    ) -> Self {
        Self {
            options,
            summary,
            replay,
            export: None,
            rendered_html: None,
            written_path: None,
            failure_error: None,
        }
    }

    pub(crate) fn take_failure_error(&mut self) -> Option<CodingSessionError> {
        self.failure_error.take()
    }

    pub(crate) fn finish_success(&self) -> Result<ExportOutcome, CodingSessionError> {
        if self.options.writes_html() && self.written_path.is_none() {
            return Err(CodingSessionError::Session {
                message: "export cannot finish without a written html path".into(),
            });
        }
        Ok(ExportOutcome {
            export: self
                .export
                .clone()
                .ok_or_else(|| CodingSessionError::Session {
                    message: "export cannot finish without an export view".into(),
                })?,
            path: self.written_path.clone(),
        })
    }

    fn fail(&mut self, error: CodingSessionError) -> String {
        let message = error.to_string();
        self.failure_error = Some(error);
        message
    }

    fn start_export(&mut self) -> Result<(), CodingSessionError> {
        if self.summary.session_id.is_empty() {
            return Err(CodingSessionError::Session {
                message: "export cannot start without a session id".into(),
            });
        }
        Ok(())
    }

    fn load_session_replay(&mut self) -> Result<(), CodingSessionError> {
        if self.replay.session_id.is_empty() {
            return Err(CodingSessionError::Session {
                message: "export cannot load an unnamed session replay".into(),
            });
        }
        if self.replay.session_id != self.summary.session_id {
            return Err(CodingSessionError::Session {
                message: format!(
                    "export replay session id '{}' does not match summary session id '{}'",
                    self.replay.session_id, self.summary.session_id
                ),
            });
        }
        Ok(())
    }

    fn select_export_view(&mut self) -> Result<(), CodingSessionError> {
        if self.export.is_none() {
            self.export = Some(export_from_replay(
                self.summary.clone(),
                self.replay.clone(),
            ));
        }
        Ok(())
    }

    fn render_export(&mut self) -> Result<(), CodingSessionError> {
        if !self.options.writes_html() {
            return Ok(());
        }
        let export = self
            .export
            .as_ref()
            .ok_or_else(|| CodingSessionError::Session {
                message: "export cannot render without an export view".into(),
            })?;
        self.rendered_html = Some(render_export_html(export));
        Ok(())
    }

    fn write_export(&mut self) -> Result<(), CodingSessionError> {
        let Some(path) = self.options.output_path().map(Path::to_path_buf) else {
            return Ok(());
        };
        let html = self
            .rendered_html
            .clone()
            .ok_or_else(|| CodingSessionError::Session {
                message: "export cannot write without rendered html".into(),
            })?;
        self.written_path = Some(write_rendered_export_html(&html, &path)?);
        Ok(())
    }

    fn emit_completion(&mut self) -> Result<(), CodingSessionError> {
        self.finish_success().map(|_| ())
    }
}

pub(crate) struct ExportFlow {
    flow: Flow<ExportContext>,
}

impl ExportFlow {
    pub(crate) fn new() -> Result<Self, CodingSessionError> {
        let mut flow = Flow::new(EXPORT_NODE_IDS[0]).map_err(flow_error)?;
        for spec in EXPORT_NODE_SPECS {
            flow.add_node(spec.id, ExportNode::new(spec.name, spec.kind))
                .map_err(flow_error)?;
        }
        crate::services::flow::add_linear_edges(&mut flow, EXPORT_NODE_IDS)?;
        Ok(Self { flow })
    }

    pub(crate) fn run(&self, ctx: &mut ExportContext) -> Result<FlowOutcome, CodingSessionError> {
        futures::executor::block_on(self.flow.run(ctx)).map_err(flow_error)
    }
}

#[derive(Debug, Clone, Copy)]
struct ExportNode {
    name: &'static str,
    kind: ExportNodeKind,
}

impl ExportNode {
    fn new(name: &'static str, kind: ExportNodeKind) -> Self {
        Self { name, kind }
    }
}

impl FlowNode<ExportContext> for ExportNode {
    fn name(&self) -> &str {
        self.name
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut ExportContext,
    ) -> Pin<Box<dyn Future<Output = Result<Action, String>> + Send + 'a>> {
        Box::pin(async move {
            let result = match self.kind {
                ExportNodeKind::StartExport => ctx.start_export(),
                ExportNodeKind::LoadSessionReplay => ctx.load_session_replay(),
                ExportNodeKind::SelectExportView => ctx.select_export_view(),
                ExportNodeKind::RenderExport => ctx.render_export(),
                ExportNodeKind::WriteExport => ctx.write_export(),
                ExportNodeKind::EmitCompletion => ctx.emit_completion(),
            };
            match result {
                Ok(()) => default_action(),
                Err(error) => Err(ctx.fail(error)),
            }
        })
    }
}

fn default_action() -> Result<Action, String> {
    Action::new(DEFAULT_ACTION).map_err(|error| error.to_string())
}

fn flow_error(error: FlowError) -> CodingSessionError {
    CodingSessionError::Flow {
        message: error.to_string(),
    }
}
