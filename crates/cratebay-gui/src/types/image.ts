/**
 * Image-related types for CrateBay.
 *
 * Matches the Tauri commands: image_list, image_search, image_pull,
 * image_remove, image_inspect, image_tag, image_export, image_import.
 */

export interface LocalImageInfo {
  id: string;
  repoTags: string[];
  sizeBytes: number;
  sizeHuman: string;
  created: number; // unix timestamp
}

export interface ImageSearchResult {
  source: string;
  reference: string;
  description: string;
  stars?: number;
  pulls?: number;
  official: boolean;
}

export interface ImageInspectInfo {
  id: string;
  repoTags: string[];
  sizeBytes: number;
  created: string;
  architecture: string;
  os: string;
  dockerVersion: string;
  layers: number;
}

export interface BundleImageLoadResult {
  imageName: string;
  tarFilename: string;
  archivePath?: string | null;
  loaded: boolean;
  skipped: boolean;
  message: string;
}
