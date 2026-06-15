export interface VolumeInfo {
  name: string;
  driver: string;
  mountpoint: string;
  createdAt?: string | null;
  scope: string;
  labels: Record<string, string>;
  options: Record<string, string>;
  managedBy: string;
}
