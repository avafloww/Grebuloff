use anyhow::{anyhow, Result};
use bzip2::read::BzDecoder;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use progress_streams::ProgressReader;
use reqwest::Client;
use std::cmp::min;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use crate::cargo_container::CargoContainer;

pub struct CefDist {
    pub version: CefVersion,
    url: String,
    filename: String,
    filename_compressed: String,
    target_dir: PathBuf,
    pub package_dir: PathBuf,
}

pub struct CefVersion(String);

impl std::fmt::Display for CefVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl CefDist {
    pub fn from(cargo: &CargoContainer<'_>) -> Result<Self> {
        let package = cargo
            .workspace
            .members()
            .find(|p| p.name() == "cef-sys")
            .ok_or(anyhow!("failed to find cef-sys in workspace"))?;

        let version = CefVersion(
            package
                .version()
                .to_string()
                .replace(".chromium", "+chromium"),
        );
        let package_dir = package.manifest_path().parent().unwrap().to_path_buf();

        let target_dir = cargo.workspace.root().join("deps");
        let filename = format!("cef_binary_{}_windows64_minimal", version);
        let filename_compressed = format!("{}.tar.bz2", filename);
        let url = format!("https://cef-builds.spotifycdn.com/{}", filename_compressed);

        Ok(Self {
            version: version,
            url: url.into(),
            filename: filename.into(),
            filename_compressed: filename_compressed.into(),
            target_dir,
            package_dir,
        })
    }

    pub fn directory(&self) -> String {
        self.target_dir
            .join(&*self.filename)
            .to_str()
            .unwrap()
            .into()
    }

    pub fn exists(&self) -> bool {
        self.target_dir.join(&*self.filename).exists()
    }

    pub async fn download(&self) -> Result<()> {
        let client = Client::new();
        let path = self.target_dir.join(&*self.filename_compressed);

        // if the compressed file exists, we don't need to download
        if path.exists() {
            return Ok(());
        }

        let path_str = path.to_str().unwrap();

        let res = client.get(&*self.url).send().await?;
        let total_size = res.content_length().expect("failed to get content length");

        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"));
        pb.set_message(format!("Downloading {}", self.url));

        // download chunks
        let mut file = File::create(path.clone())?;
        let mut downloaded: u64 = 0;
        let mut stream = res.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            file.write_all(&chunk)?;
            let new = min(downloaded + (chunk.len() as u64), total_size);
            downloaded = new;
            pb.set_position(new);
        }

        pb.finish_with_message(format!("Downloaded {} to {}", self.url, path_str));

        Ok(())
    }

    pub fn extract(&self) -> Result<PathBuf> {
        let path = self.target_dir.join(&*self.filename);
        let path_str = path.to_str().unwrap();

        // if the extracted folder exists, we don't need to extract
        if path.exists() {
            return Ok(path);
        }

        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-")
        );

        pb.set_message(format!("Extracting CEF binaries to {}", path_str));

        // extract the compressed file
        let tar_bz = File::open(self.target_dir.join(&*self.filename_compressed))?;
        let compressed_size = tar_bz.metadata()?.len();

        pb.set_length(compressed_size);

        let mut total_read: u64 = 0;
        let progress_reader = ProgressReader::new(tar_bz, |newly_read| {
            total_read += newly_read as u64;
            pb.set_position(total_read);
        });

        let tar = BzDecoder::new(progress_reader);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(self.target_dir.clone())?;

        pb.finish_with_message(format!(
            "Extracted {} to {}",
            self.filename_compressed, path_str
        ));

        Ok(path)
    }
}
