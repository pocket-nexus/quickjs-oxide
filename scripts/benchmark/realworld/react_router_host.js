import React from "react";

let current = { loader: {}, params: {}, location: { search: "" } };
const submissions = [];
export function setRouteState(next) { current = next; }
export function takeSubmissions() { return submissions.splice(0); }
export function useLoaderData() { return current.loader; }
export function useParams() { return current.params; }
export function useLocation() { return current.location; }
export function useAsyncError() { return null; }
export function isRouteErrorResponse(value) { return Boolean(value && typeof value.status === "number"); }
export function Await({ resolve, children }) { return children(resolve); }
export function Link({ to, children, ...props }) {
  const href = typeof to === "string" ? to : to.search || "";
  return React.createElement("a", { ...props, href }, children);
}
export function useFetcher({ key } = {}) {
  const Form = ({ children, ...props }) => React.createElement("form", props, children);
  return {
    state: "idle", formData: null, data: null, Form,
    submit(data, options) { submissions.push({ key, data, options }); }
  };
}
