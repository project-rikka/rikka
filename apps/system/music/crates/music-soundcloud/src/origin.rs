use std::sync::Arc;

use futures_util::future::try_join;
use music_core::{Error, Id, Quality, Result, async_trait};
use music_stream::{Origin, Plan, Spool};
use serde_json::Value;

use crate::api::Api;
use crate::cdn::Cdn;
use crate::parse::text;
use crate::transcoding::{self, Delivery};
use crate::{hls, mp4};

pub(crate) struct SoundCloudOrigin {
    api: Arc<Api>,
    cdn: Cdn,
}

impl SoundCloudOrigin {
    pub(crate) fn new(api: Arc<Api>) -> SoundCloudOrigin {
        SoundCloudOrigin {
            api,
            cdn: Cdn::new(),
        }
    }

    async fn located(&self, track: &Id, quality: Quality) -> Result<(Delivery, String)> {
        let answer = self.api.get(&format!("tracks/{track}"), &[]).await?;
        let chosen = transcoding::choose(&answer, quality)?;
        let mut query = Vec::new();
        if let Some(authorization) = text(&answer, "track_authorization") {
            query.push(("track_authorization", authorization.to_owned()));
        }
        let located = self.api.get_at(&chosen.url, &query).await?;
        let url = located
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Unreadable(format!("no stream address for track {track}")))?;
        Ok((chosen.delivery, url.to_owned()))
    }

    async fn segmented(&self, url: &str) -> Result<Plan> {
        let listed = hls::listing(&self.cdn.text(url).await?, url)?;
        let Some(init) = &listed.init else {
            return self.cdn.segments(&listed.segments).await;
        };
        let [first, rest @ ..] = listed.segments.as_slice() else {
            return Err(Error::Unreadable("the stream has no segments".into()));
        };
        let head = async {
            let (init, first) = try_join(self.cdn.bytes(init), self.cdn.bytes(first)).await?;
            Ok(mp4::with_duration([init, first].concat(), listed.seconds))
        };
        let (head, rest) = try_join(head, self.cdn.segments(rest)).await?;
        Ok(rest.lead(head))
    }
}

#[async_trait]
impl Origin for SoundCloudOrigin {
    async fn fetch(&self, track: &Id, quality: Quality, spool: &mut Spool) -> Result<()> {
        let (delivery, url) = self.located(track, quality).await?;
        let plan = match delivery {
            Delivery::Progressive => self.cdn.file(&url).await?,
            Delivery::Segmented => self.segmented(&url).await?,
        };
        self.cdn.download(plan, spool).await
    }
}
