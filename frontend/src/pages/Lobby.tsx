import { Link } from "react-router";

export function Lobby() {
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
