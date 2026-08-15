import { useCallback, useMemo, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { apiPost } from "@/lib/api/client";
import { routes } from "@/lib/api-routes";
import { serverKeys } from "@/lib/query-keys";
import { formatJsonImport, getJsonImportDiagnostics } from "@/lib/json-import-editor";
import type {
  ExecuteImportInput,
  ExecuteImportResult,
  ScannedServer,
  ImportPreview as ImportPreviewType,
} from "@moor/types";

type ImportPreview = ImportPreviewType;

export function useConfigImport() {
  const queryClient = useQueryClient();

  const [scanCandidates, setScanCandidates] = useState<ScannedServer[]>([]);
  const [selectedImports, setSelectedImports] = useState<Set<string>>(new Set());
  const [selectedProfileIds, setSelectedProfileIds] = useState<string[]>([]);
  const [scanStatus, setScanStatus] = useState<string | null>(null);
  const [importPreview, setImportPreview] = useState<ImportPreview | null>(null);

  const [jsonImport, setJsonImport] = useState("");
  const [jsonImportErrors, setJsonImportErrors] = useState<string[]>([]);
  const [jsonImportStatus, setJsonImportStatus] = useState<string | null>(null);
  const jsonImportDiagnostics = useMemo(() => getJsonImportDiagnostics(jsonImport), [jsonImport]);

  const applyImportPreview = useCallback((result: ImportPreview) => {
    setImportPreview(result);
    setScanCandidates(result.servers);
    setSelectedImports(new Set(result.servers.map((server) => server.name)));
    setSelectedProfileIds([]);
    setScanStatus(
      result.newServers === 0 ? `Scanned ${result.scanned} configs. No new servers found.` : null,
    );
  }, []);

  const scan = useCallback(async () => {
    try {
      const result = await apiPost<ImportPreview>(routes.import.scan(), {});
      applyImportPreview(result);
    } catch (err) {
      setScanStatus((err as Error).message);
    }
  }, [applyImportPreview]);

  const updateJsonImport = useCallback((value: string) => {
    setJsonImport(value);
    setJsonImportErrors([]);
    setJsonImportStatus(null);
  }, []);

  const formatJson = useCallback(() => {
    const result = formatJsonImport(jsonImport);
    setJsonImportErrors([]);

    if (result.diagnostics.length > 0) {
      setJsonImportStatus("Fix JSON syntax errors before formatting.");
      return;
    }

    setJsonImport(result.value);
    setJsonImportStatus(result.formatted ? "JSON formatted." : "JSON is already formatted.");
  }, [jsonImport]);

  const parseJson = useCallback(async () => {
    if (jsonImportDiagnostics.length > 0) {
      setJsonImportErrors([]);
      setJsonImportStatus("Fix JSON syntax errors before previewing.");
      return;
    }

    try {
      const result = await apiPost<ImportPreview>(routes.import.parse(), { content: jsonImport });
      if (result.errors.length > 0 || (result.diagnostics?.length ?? 0) > 0) {
        setImportPreview(result);
        setScanCandidates([]);
        setSelectedImports(new Set());
        setJsonImportErrors(result.errors);
        setJsonImportStatus(null);
        return;
      }

      applyImportPreview(result);
      setJsonImportErrors([]);
      setJsonImportStatus(null);
      return true;
    } catch (err) {
      setJsonImportErrors([(err as Error).message]);
      setJsonImportStatus(null);
      return false;
    }
  }, [jsonImport, jsonImportDiagnostics, applyImportPreview]);

  const executeImportMutation = useMutation({
    mutationFn: async () => {
      const serversToImport = scanCandidates.filter((server) => selectedImports.has(server.name));
      const input: ExecuteImportInput = {
        servers: serversToImport,
        profileIds: selectedProfileIds,
      };
      const result = await apiPost<ExecuteImportResult>(routes.import.execute(), input);
      return result;
    },
    onSuccess: (result) => {
      setScanStatus(
        `Imported ${result.imported.length} servers. Skipped ${result.skipped.length}.`,
      );
      setScanCandidates([]);
      setSelectedImports(new Set());
      setSelectedProfileIds([]);
      setImportPreview(null);
      void queryClient.invalidateQueries({ queryKey: serverKeys.list() });
    },
  });

  const toggleImport = useCallback((name: string, checked: boolean) => {
    setSelectedImports((prev) => {
      const next = new Set(prev);
      if (checked) next.add(name);
      else next.delete(name);
      return next;
    });
  }, []);

  const clearScan = useCallback(() => {
    setScanCandidates([]);
    setScanStatus(null);
    setImportPreview(null);
    setSelectedProfileIds([]);
  }, []);

  const clearJsonImport = useCallback(() => {
    setJsonImport("");
    setJsonImportErrors([]);
    setJsonImportStatus(null);
  }, []);

  const hasStaticAuthorizationHeader = scanCandidates.some((server) => {
    const authorization = Object.entries(server.headers ?? {}).find(
      ([key]) => key.toLowerCase() === "authorization",
    )?.[1];
    return Boolean(authorization && !authorization.includes("{env:"));
  });

  return {
    scanCandidates,
    selectedImports,
    selectedProfileIds,
    scanStatus,
    importPreview,
    hasStaticAuthorizationHeader,
    jsonImport,
    jsonImportErrors,
    jsonImportStatus,
    jsonImportDiagnostics,
    jsonImportStatusIsError: jsonImportStatus?.startsWith("Fix ") ?? false,
    scan,
    updateJsonImport,
    formatJson,
    parseJson,
    executeImport: executeImportMutation.mutateAsync,
    setSelectedProfileIds,
    toggleImport,
    clearScan,
    clearJsonImport,
  };
}
