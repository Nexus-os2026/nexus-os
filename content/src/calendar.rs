use crate::generator::{PlatformContent, SocialPlatform};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledPost {
    pub id: String,
    pub platform: SocialPlatform,
    pub datetime: String,
    pub content: PlatformContent,
}

#[derive(Debug)]
pub struct ContentCalendar {
    storage_path: PathBuf,
    posts: Vec<ScheduledPost>,
}

impl ContentCalendar {
    pub fn new(storage_path: impl AsRef<Path>) -> Result<Self, AgentError> {
        let storage_path = storage_path.as_ref().to_path_buf();
        let posts = if storage_path.exists() {
            let data = fs::read_to_string(storage_path.as_path()).map_err(|error| {
                AgentError::SupervisorError(format!("failed reading calendar store: {error}"))
            })?;
            if data.trim().is_empty() {
                Vec::new()
            } else {
                serde_json::from_str(data.as_str()).map_err(|error| {
                    AgentError::SupervisorError(format!("failed parsing calendar store: {error}"))
                })?
            }
        } else {
            Vec::new()
        };

        Ok(Self {
            storage_path,
            posts,
        })
    }

    pub fn schedule_post(
        &mut self,
        content: PlatformContent,
        platform: SocialPlatform,
        datetime: &str,
    ) -> Result<ScheduledPost, AgentError> {
        let scheduled = ScheduledPost {
            id: Uuid::new_v4().to_string(),
            platform,
            datetime: datetime.to_string(),
            content,
        };

        self.posts.push(scheduled.clone());
        self.persist()?;
        Ok(scheduled)
    }

    pub fn list_upcoming(&self) -> Vec<ScheduledPost> {
        let mut posts = self.posts.clone();
        posts.sort_by(|left, right| left.datetime.cmp(&right.datetime));
        posts
    }

    pub fn cancel_post(&mut self, id: &str) -> Result<bool, AgentError> {
        let before = self.posts.len();
        self.posts.retain(|post| post.id != id);
        let changed = self.posts.len() != before;
        if changed {
            self.persist()?;
        }
        Ok(changed)
    }

    fn persist(&self) -> Result<(), AgentError> {
        let serialized = serde_json::to_string_pretty(&self.posts).map_err(|error| {
            AgentError::SupervisorError(format!("failed serializing calendar posts: {error}"))
        })?;
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AgentError::SupervisorError(format!(
                    "failed creating calendar storage directory: {error}"
                ))
            })?;
        }
        fs::write(self.storage_path.as_path(), serialized).map_err(|error| {
            AgentError::SupervisorError(format!("failed writing calendar store: {error}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ContentCalendar;
    use crate::generator::{PlatformContent, SocialPlatform};
    use tempfile::tempdir;

    fn sample_content(platform: SocialPlatform, text: &str) -> PlatformContent {
        PlatformContent {
            platform,
            text: text.to_string(),
            hashtags: vec!["#rust".to_string()],
            thread: None,
            image_prompt: None,
            link_preview: None,
        }
    }

    #[test]
    fn test_content_calendar_schedule() {
        let temp_dir = tempdir().expect("temp directory should be created");
        let file_path = temp_dir.path().join("nexus-calendar.json");

        let mut calendar = match ContentCalendar::new(file_path.as_path()) {
            Ok(calendar) => calendar,
            Err(error) => panic!("calendar should initialize: {error}"),
        };

        let post1 = calendar.schedule_post(
            sample_content(SocialPlatform::X, "x post"),
            SocialPlatform::X,
            "2026-01-01T10:00:00Z",
        );
        let post2 = calendar.schedule_post(
            sample_content(SocialPlatform::Instagram, "ig post"),
            SocialPlatform::Instagram,
            "2026-01-01T11:00:00Z",
        );
        let post3 = calendar.schedule_post(
            sample_content(SocialPlatform::Facebook, "fb post"),
            SocialPlatform::Facebook,
            "2026-01-01T12:00:00Z",
        );

        assert!(post1.is_ok());
        assert!(post2.is_ok());
        assert!(post3.is_ok());

        let listed = calendar.list_upcoming();
        assert_eq!(listed.len(), 3);

        if let Ok(post) = post2 {
            let canceled = calendar.cancel_post(post.id.as_str());
            assert_eq!(canceled, Ok(true));
        }

        let listed_after = calendar.list_upcoming();
        assert_eq!(listed_after.len(), 2);
    }
}
