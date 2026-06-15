export interface NetworkInfo {
  id: string;
  name: string;
  driver: string;
  scope: string;
  internal: boolean;
  attachable: boolean;
  labels: Record<string, unknown>;
  containers: Record<string, unknown>;
  managedBy: string;
}
