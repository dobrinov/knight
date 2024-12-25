import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { createBrowserRouter, RouterProvider } from "react-router";
import "./index.css";
import { CreateGame } from "./pages/CreateGame";
import { Landingpage } from "./pages/Landingpage";
import { Lobby } from "./pages/Lobby";

const router = createBrowserRouter([
  { path: "/", element: <Landingpage /> },
  { path: "/lobby", element: <Lobby /> },
  { path: "/games/new", element: <CreateGame /> },
]);

const api = new QueryClient();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={api}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>
);
