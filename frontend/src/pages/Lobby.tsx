import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";

export function Lobby() {
  const {
    isPending,
    error,
    data: lobbies,
  } = useQuery({
    queryKey: ["lobby"],
    queryFn: () =>
      fetch("http://localhost:4567/api/lobby").then((res) => res.json()),
  });

  if (isPending) return "Loading...";

  if (error) return "An error has occurred: " + error.message;

  return (
    <div>
      <strong>Lobby</strong>
      <Link to="/games/new">Create game</Link>

      <table border={1}>
        <thead>
          <tr>
            <th>Name</th>
            <th>Players</th>
            <th>Map</th>
            <th>&nbsp;</th>
          </tr>
        </thead>
        <tbody>
          {lobbies.map((lobby: any) => (
            <tr key={lobby.id}>
              <td>Hello world</td>
              <td>
                {lobby.players.length}/{lobby.max_players}
              </td>
              <td>{lobby.map}</td>
              <td>
                <button type="button">Join</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
