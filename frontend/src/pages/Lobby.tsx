import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";

export function Lobby() {
  const { isPending, error, data } = useQuery({
    queryKey: ["repoData"],
    queryFn: () =>
      fetch("http://localhost:4567/api/lobby").then((res) => res.json()),
  });

  if (isPending) return "Loading...";

  if (error) return "An error has occurred: " + error.message;

  console.log(data);

  return (
    <div>
      <strong>Lobby</strong>
      <Link to="/games/new">Create game</Link>

      <table border={1}>
        <thead>
          <tr>
            <th>Name</th>
            <th>Players</th>
            <th>&nbsp;</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>Hello world</td>
            <td>2/4</td>
            <td>
              <button type="button">Join</button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  );
}
