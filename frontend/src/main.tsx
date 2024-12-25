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

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>
);
