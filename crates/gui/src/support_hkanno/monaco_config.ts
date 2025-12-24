import { OnMount } from "@monaco-editor/react";
import { open } from "@tauri-apps/api/shell";

/**
 * NOTE: By default, the URL is opened in the app, so prevent this and call the backend API to open the URL in the browser of each PC.
 * @param _editor
 * @param monacoEnv
 */
export const setMonacoEditorConfig: OnMount = (_editor, monacoEnv) => {
  if (window.__TAURI__) {
    monacoEnv.editor.registerLinkOpener({
      open(url: string) {
        open(url.toString());
        //? False is for hooks, but true replaces the function.
        //? In this case, it is a replacement because it opens the URL with its own API.
        return true;
      },
    });
  }
};
